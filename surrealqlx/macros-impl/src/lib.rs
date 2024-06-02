use std::borrow::Borrow;

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse::Parse, punctuated::Punctuated, Data, DeriveInput};

#[cfg(test)]
mod tests;

pub fn table_macro_impl(input: TokenStream) -> TokenStream {
    let input = match syn::parse2::<DeriveInput>(input) {
        Ok(input) => input,
        Err(err) => {
            return err.to_compile_error().into();
        }
    };

    let struct_name = &input.ident;

    let table_name = match parse_table_name(&input) {
        Ok(table_name) => table_name,
        Err(err) => {
            return err.to_compile_error().into();
        }
    };

    let struct_fields = match parse_struct_fields(&input) {
        Ok(fields) => fields,
        Err(err) => {
            return err.to_compile_error().into();
        }
    };

    let (table_field_queries, index_queries) =
        match create_table_field_queries(struct_fields, &table_name) {
            Ok(queries) => queries,
            Err(err) => {
                return err.to_compile_error().into();
            }
        };

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
    expanded.into()
}

// I want to make a derive macro that will implement the Table trait for a struct
// with the given table_name.
//
// the name of the table will be given by an attribute #[Table("table_name")]
//
// the table trait defines a const TABLE_NAME: &'static str, and a fn init_table() -> String
// the function returns a string that is a SurrealQL query that creates the table
//
// each field in the struct will be a field in the table, which will be created with the same name, and the type given by the `#[field]` attribute.
//
// if a field is missing the `#[field]` attribute, the macro will return an error, unless it is annotated #[field(ignore)] and is either an `Option`, or has a default value.
//
// the macro will also implement the Table trait for the struct

#[derive(Default)]
struct VectorIndexAnnotation {
    dim: Option<usize>,
}

impl VectorIndexAnnotation {
    fn parse(args: &Punctuated<syn::Expr, syn::token::Comma>) -> syn::Result<Self> {
        let mut vectorindex = Self::default();
        for arg in args {
            match arg {
                syn::Expr::Assign(assign)
                    if assign.left.to_token_stream().to_string().eq("dim") =>
                {
                    if let syn::Expr::Lit(lit) = &*assign.right {
                        if let syn::Lit::Int(int) = &lit.lit {
                            vectorindex.dim = Some(int.base10_parse()?);
                            continue;
                        }
                    }
                    return Err(syn::Error::new_spanned(assign, "Unsupported right operand, `dim` expects an integer literal representing the number of dimensions in the vector"));
                }
                syn::Expr::Lit(lit) => {
                    match &lit.lit {
                        syn::Lit::Int(int) => vectorindex.dim = Some(int.base10_parse()?),
                        _ => return Err(syn::Error::new_spanned(lit, "`dim` expects an integer literal representing the number of dimensions in the vector")),
                    }
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        arg,
                        "Unsupported expression syntax",
                    ))
                }
            }
        }

        if vectorindex.dim.is_none() {
            return Err(syn::Error::new_spanned(
                args,
                "vector attribute without dimension set",
            ));
        }

        Ok(vectorindex)
    }
}

#[derive(Default)]
struct IndexAnnotation {
    unique: bool,
    vector: VectorIndexAnnotation,
}

impl IndexAnnotation {
    fn to_query_string(&self, table_name: &str, field_name: &str) -> String {
        format!(
            "DEFINE INDEX {table_name}_{field_name}_index ON {table_name} FIELDS {field_name}{};",
            {
                let mut extra = String::new();
                if self.unique {
                    extra.push_str(" UNIQUE");
                }
                if let Some(vector_dim) = self.vector.dim {
                    extra.push_str(format!(" MTREE DIMENSION {vector_dim}").as_str());
                }

                extra
            }
        )
    }
    fn parse(args: &Punctuated<syn::Expr, syn::token::Comma>) -> syn::Result<Self> {
        let mut index = Self::default();
        for arg in args {
            match arg {
                syn::Expr::Call(call) if call.func.to_token_stream().to_string().eq("vector") => {
                    index.vector = VectorIndexAnnotation::parse(&call.args)?;
                }
                syn::Expr::Path(path) if path.to_token_stream().to_string().eq("unique") => {
                    index.unique = true;
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        arg,
                        "Unsupported expression syntax",
                    ))
                }
            }
        }

        Ok(index)
    }
}

struct FieldAnnotation {
    skip: bool,
    type_: Option<syn::LitStr>,
    index: Option<IndexAnnotation>,
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
        let mut index = None;

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
                        index = Some(IndexAnnotation::parse(call.args.borrow())?);
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

            if let Ok(dt) = input.parse::<syn::LitStr>() {
                // is this a string literal param? (i.e., the type)
                // `#[field("foobar")]`
                type_ = Some(dt);
            } else if let Ok(ident) = input.parse::<syn::Ident>() {
                // is this an ident param
                // `#[field(skip)]`
                match ident.to_string().as_str() {
                    "skip" => {
                        skip = true;
                        break;
                    }
                    "dt" => {
                        input.parse::<syn::Token![=]>()?;
                        type_ = Some(input.parse::<syn::LitStr>()?);
                    }
                    s => {
                        // if it is neither `skip` nor `type`, try to parse a litstr as the type or return an error
                        return Err(syn::Error::new_spanned(
                            ident,
                            format!(
                                "Unknown field attribute, expected `skip` or `dt`, found `{s}`"
                            ),
                        ));
                    }
                }
            } else if let Ok(expr_call) = input.parse::<syn::ExprCall>() {
                // is this in call expression syntax?
                // `#[field(index(unique, vector(dim= 7))]`
                match expr_call.func.to_token_stream().to_string().as_str() {
                    "index" => {
                        index = Some(IndexAnnotation::parse(expr_call.args.borrow())?);
                    }
                    _ => {
                        return Err(syn::Error::new_spanned(
                            expr_call.func,
                            "Unknown field attribute",
                        ))
                    }
                }
            }

            let _ = input.parse::<syn::Token![,]>();
        }

        Ok(Self { skip, type_, index })
    }
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

        let mut field_index = None;

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
                if index.is_some() {
                    field_index = index;
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

        if let Some(index) = field_index {
            index_queries.push(index.to_query_string(table_name, &field_name.to_string()));
        }
    }

    Ok((table_field_queries, index_queries))
}
