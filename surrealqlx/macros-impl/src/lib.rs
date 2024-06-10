use std::borrow::Borrow;

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse::Parse, punctuated::Punctuated, Data, DeriveInput, ExprAssign, ExprLit};

#[cfg(test)]
mod tests;

/// Implementation of the Table derive macro
///
/// # Errors
///
/// This function will return an error if the input couldn't be parsed, or if attributes are missing or invalid.
pub fn table_macro_impl(input: TokenStream) -> syn::Result<TokenStream> {
    let input = syn::parse2::<DeriveInput>(input)?;

    let struct_name = &input.ident;

    let table_name = parse_table_name(&input)?;

    let struct_fields = parse_struct_fields(&input)?;

    let (table_field_queries, index_queries) =
        create_table_field_queries(struct_fields, &table_name)?;

    let table_query = format!("DEFINE TABLE {table_name} SCHEMAFULL;");

    let table_field_queries = table_field_queries.iter().map(|q| quote! {.query(#q)});
    let index_queries = index_queries.iter().map(|q| quote! {.query(#q)});

    // Build the output, possibly using the input
    let expanded = quote! {
        // The generated impl goes here
        impl ::surrealqlx::traits::Table for #struct_name {
            const TABLE_NAME: &'static str = #table_name;

            #[allow(manual_async_fn)]
            fn init_table<C: ::surrealdb::Connection>(
                db: &::surrealdb::Surreal<C>,
            ) -> impl ::std::future::Future<Output = ::surrealdb::Result<()>> + Send {
                async {
                    let _ = db.query("BEGIN;")
                        .query(#table_query)
                        .query("COMMIT;")
                        .query("BEGIN;")
                        #(
                            #table_field_queries
                        )*
                        .query("COMMIT;")
                        .query("BEGIN;")
                        #(
                            #index_queries
                        )*
                        .query("COMMIT;").await?;
                    Ok(())
                }
            }
        }
    };

    // Hand the output tokens back to the compiler
    Ok(expanded)
}

fn parse_table_name(input: &DeriveInput) -> syn::Result<String> {
    let table_name = input
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("Table"))
        .ok_or_else(|| {
            syn::Error::new_spanned(input, "Table attribute must be specified for the struct")
        })
        .and_then(|attr| attr.parse_args::<syn::LitStr>().map(|lit| lit.value()))?;
    Ok(table_name)
}

fn parse_struct_fields(input: &DeriveInput) -> syn::Result<impl Iterator<Item = &syn::Field>> {
    match input.data {
        Data::Struct(ref data) => match data.fields {
            syn::Fields::Named(ref fields) => {
                let mut fields = fields.named.iter().peekable();
                if fields.peek().is_none() {
                    return Err(syn::Error::new_spanned(
                        input,
                        "Struct must have at least one field",
                    ));
                }
                Ok(fields)
            }
            _ => Err(syn::Error::new_spanned(
                input,
                "Tuple structs not supported",
            )),
        },
        _ => Err(syn::Error::new_spanned(input, "Only structs are supported")),
    }
}

fn create_table_field_queries<'a>(
    fields: impl Iterator<Item = &'a syn::Field>,
    table_name: &str,
) -> syn::Result<(Vec<String>, Vec<String>)> {
    let mut table_field_queries = Vec::new();

    let mut index_queries = Vec::new();

    for field in fields {
        let Some(field_name) = field.ident.as_ref() else {
            return Err(syn::Error::new_spanned(
                field,
                "Field must have a name, tuple structs not allowed",
            ));
        };
        let mut field_attrs = field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("field"))
            .map(|attr| {
                let parsed = attr.parse_args::<FieldAnnotation>();
                match parsed {
                    Ok(parsed) => Ok((attr, parsed)),
                    Err(err) => Err(err),
                }
            })
            .peekable();

        let mut field_indexes = Vec::new();

        // process the field attribute
        let field_type = match field_attrs.next() {
            Some(Ok((_, FieldAnnotation { skip: true, .. }))) => {
                continue;
            }
            Some(Ok((
                _,
                FieldAnnotation {
                    skip: false,
                    type_: Some(type_),
                    index,
                },
            ))) => {
                if !index.is_empty() {
                    field_indexes = index;
                };
                type_.value()
            }
            Some(Ok((
                field_attr,
                FieldAnnotation {
                    skip: false,
                    type_: None,
                    ..
                },
            ))) => {
                return Err(syn::Error::new_spanned(
                    field_attr,
                    "Field must have a type specified in the #[field] attribute",
                ));
            }
            Some(Err(err)) => {
                return Err(err);
            }
            None => {
                return Err(syn::Error::new_spanned(
                    field,
                    "Field must have a #[field] attribute",
                ));
            }
        };
        // next, make sure there was only one field attribute
        if field_attrs.peek().is_some() {
            return Err(syn::Error::new_spanned(
                field,
                "Field can have only one #[field] attribute",
            ));
        }

        table_field_queries.push(format!(
            "DEFINE FIELD {field_name} ON {table_name} TYPE {field_type};",
        ));

        for index in field_indexes {
            index_queries.push(index.to_query_string(table_name, &field_name.to_string()));
        }
    }

    Ok((table_field_queries, index_queries))
}

struct FieldAnnotation {
    skip: bool,
    type_: Option<syn::LitStr>,
    index: Vec<IndexAnnotation>,
}

/// parses the `#[field]` attribute
///
/// the `#[field]` attribute can have the following keys:
/// - `skip`: if set, the field will be skipped
/// - `type`: the type of the field
impl Parse for FieldAnnotation {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut skip = false;
        let mut type_ = None;
        let mut index = Vec::new();

        // TODO: error if more than one of the same type of index is specified

        while !input.is_empty() {
            match input.parse::<syn::Expr>()? {
                syn::Expr::Assign(assign) => match assign.left.to_token_stream().to_string().as_str() {
                    "dt" => match *assign.right {
                        syn::Expr::Lit(lit)=>match lit.lit {
                            syn::Lit::Str(strlit) => type_=Some(strlit),
                            l => return Err(syn::Error::new_spanned(l, "unexpected literal, the `dt` attribute expects a string literal")),
                        },
                        rhs => return Err(syn::Error::new_spanned(rhs,"unexpected expression, the `dt` attribute expects a string literal")),
                    }
                    _ =>
                    return Err(syn::Error::new_spanned(
                        assign.left,
                        "Unknown field attribute",
                    ))
                },
                syn::Expr::Call(call) => match call.func.to_token_stream().to_string().as_str() {
                    "index" => {
                        index.push(IndexAnnotation::parse(call.args.borrow())?);
                    }
                    _ => {
                        return Err(syn::Error::new_spanned(
                            call.func,
                            "Unknown field attribute",
                        ))
                    }
                },
                syn::Expr::Lit(lit) => match lit.lit {
                    syn::Lit::Str(strlit) => type_=Some(strlit),
                    l => return Err(syn::Error::new_spanned(l, "unexpected literal")),
                },
                syn::Expr::Path(path) => match path.to_token_stream().to_string().as_str() {
                    "skip" => {
                        skip = true;
                        break;
                    }
                    s => {
                        // if it is neither `skip` nor `type`, try to parse a litstr as the type or return an error
                        return Err(syn::Error::new_spanned(
                            path,
                            format!(
                                "Unknown field attribute, expected `skip` or `dt`, found `{s}`"
                            ),
                        ));
                    }
                }
                expr => return Err(syn::Error::new_spanned(expr, "Unexpected expression syntax found, attribute parameters should be in the forms: `foo`, `\"foo\"`, `foo = ...`, or `foo(...)`")),
            }

            let _ = input.parse::<syn::Token![,]>();
        }

        Ok(Self { skip, type_, index })
    }
}

#[derive(Default, Debug, Clone)]
struct IndexAnnotation {
    compound: CompoundIndexAnnotation,
    index_type: IndexTypeAnnotation,
}

impl IndexAnnotation {
    // if both vector and full-text indexes are set, return None
    fn to_query_string(&self, table_name: &str, field_name: &str) -> String {
        let (extra, index_type) = match &self.index_type {
            IndexTypeAnnotation::Vector(vector) => {
                (format!(" MTREE DIMENSION {}", vector.dim), "vector")
            }
            IndexTypeAnnotation::Text(text) => {
                (format!(" SEARCH ANALYZER {} BM25", text.analyzer), "text")
            }
            IndexTypeAnnotation::Normal => (String::new(), "normal"),
            IndexTypeAnnotation::Unique => (String::from(" UNIQUE"), "unique"),
        };
        let index_name = format!(
            "{table_name}_{field_name}{compound_fields}_{index_type}_index",
            compound_fields = if self.compound.is_empty() {
                String::new()
            } else {
                format!("_{}", self.compound.join("_"))
            }
        );

        format!(
            "DEFINE INDEX {index_name} ON {table_name} FIELDS {field_name}{compound_fields}{extra};",
            compound_fields = if self.compound.is_empty() {
                String::new()
            } else {
                format!(",{}", self.compound.join(","))
            }
        )
    }
    fn parse(args: &Punctuated<syn::Expr, syn::token::Comma>) -> syn::Result<Self> {
        let mut args_iter = args.iter();
        // check first arg, if it is a call expr to "compound", parse the compound index and try to parse the index type from the next arg

        // check first arg, either compound or an index type
        let (compound, index_type) = match args_iter.next() {
            // if compound, parse the compound index and try to parse the index type from the next arg
            Some(syn::Expr::Call(call))
                if call.func.to_token_stream().to_string().eq("compound") =>
            {
                (
                    Some(CompoundIndexAnnotation::parse(&call.args)?),
                    IndexTypeAnnotation::parse(args_iter.next())?,
                )
            }
            // if not compound, parse the index type from the arg
            arg => (None, IndexTypeAnnotation::parse(arg)?),
        };
        // next, check the next arg, it should be compound, otherwise we have an error
        let compound = match args_iter.next() {
            Some(syn::Expr::Call(call))
                if call.func.to_token_stream().to_string().eq("compound") =>
            {
                CompoundIndexAnnotation::parse(&call.args)?
            }
            Some(arg) => {
                return Err(syn::Error::new_spanned(
                    arg,
                    "unexpected parameters in index attribute",
                ))
            }
            None => compound.unwrap_or_default(),
        };

        Ok(Self {
            compound,
            index_type,
        })
    }
}

#[derive(Default, Debug, Clone)]
struct CompoundIndexAnnotation {
    compound_fields: Vec<String>,
}

impl CompoundIndexAnnotation {
    fn parse(args: &Punctuated<syn::Expr, syn::token::Comma>) -> syn::Result<Self> {
        let mut compound_fields = Vec::new();

        for arg in args {
            match arg {
                syn::Expr::Lit(ExprLit {
                    lit: syn::Lit::Str(strlit),
                    ..
                }) => compound_fields.push(strlit.value()),
                _ => {
                    return Err(syn::Error::new_spanned(
                        arg,
                        "Compound index attribute expects string literals representing the other field names",
                    ))
                }
            }
        }

        if compound_fields.is_empty() {
            Err(syn::Error::new_spanned(
                args,
                "Compound index attribute expects at least one string literal representing the other field names",
            ))
        } else {
            Ok(Self { compound_fields })
        }
    }

    fn is_empty(&self) -> bool {
        self.compound_fields.is_empty()
    }

    fn join(&self, sep: &str) -> String {
        self.compound_fields.join(sep)
    }
}

#[derive(Default, Debug, Clone)]
enum IndexTypeAnnotation {
    Vector(VectorIndexAnnotation),
    Text(TextIndexAnnotation),
    #[default]
    Normal,
    Unique,
}

impl IndexTypeAnnotation {
    fn parse(arg: Option<&syn::Expr>) -> syn::Result<Self> {
        match arg {
            None => Ok(Self::Normal),
            Some(syn::Expr::Path(path)) if path.to_token_stream().to_string().eq("unique") => {
                Ok(Self::Unique)
            }
            Some(syn::Expr::Call(call)) if call.func.to_token_stream().to_string().eq("vector") => {
                Ok(Self::Vector(VectorIndexAnnotation::parse(&call.args)?))
            }
            Some(syn::Expr::Call(call)) if call.func.to_token_stream().to_string().eq("text") => {
                Ok(Self::Text(TextIndexAnnotation::parse(&call.args)?))
            }
            _ => Err(syn::Error::new_spanned(
                arg,
                "Unsupported expression syntax",
            )),
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct VectorIndexAnnotation {
    dim: usize,
}

impl VectorIndexAnnotation {
    fn parse(args: &Punctuated<syn::Expr, syn::token::Comma>) -> syn::Result<Self> {
        let mut args_iter = args.iter();
        let arg = args_iter.next();
        if args_iter.next().is_some() {
            return Err(syn::Error::new_spanned(
                args,
                "Vector index attribute only expects one argument, the dimension of the vector",
            ));
        }

        let dim = match arg {
            Some(syn::Expr::Assign(ExprAssign { left, right, .. }))
                if left.to_token_stream().to_string().eq("dim") =>
            {
                match *right.to_owned() {
                    syn::Expr::Lit(ExprLit {
                        lit: syn::Lit::Int(int),
                        ..
                    }) => int.base10_parse()?,
                    _ => {
                        return Err(syn::Error::new_spanned(right, "`dim` expects an integer literal representing the number of dimensions in the vector"));
                    }
                }
            }
            Some(syn::Expr::Lit(ExprLit {
                lit: syn::Lit::Int(int),
                ..
            })) => int.base10_parse()?,
            _ => {
                return Err(syn::Error::new_spanned(
                    arg,
                    "Unsupported expression syntax",
                ))
            }
        };

        if dim < 1 {
            return Err(syn::Error::new_spanned(
                arg,
                "Vector dimension must be greater than 0",
            ));
        }

        Ok(Self { dim })
    }
}

#[derive(Debug, Clone)]
struct TextIndexAnnotation {
    analyzer: String,
}

impl TextIndexAnnotation {
    fn parse(args: &Punctuated<syn::Expr, syn::token::Comma>) -> syn::Result<Self> {
        // should only read one argument, the analyzer (string literal)
        let mut args_iter = args.iter();
        let arg = args_iter.next();

        if args_iter.next().is_some() {
            return Err(syn::Error::new_spanned(
                args,
                "Text index attribute only expects one argument, the analyzer to use",
            ));
        }

        let analyzer = match arg {
            Some(syn::Expr::Lit(ExprLit {
                lit: syn::Lit::Str(strlit),
                ..
            })) => strlit.value(),
            _ => return Err(syn::Error::new_spanned(
                arg,
                "Text index attribute expects a string literal representing the analyzer to use",
            )),
        };

        Ok(Self { analyzer })
    }
}
