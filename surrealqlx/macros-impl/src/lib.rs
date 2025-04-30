use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Data, DeriveInput, ExprAssign, ExprLit, parse::Parse, punctuated::Punctuated};

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

    let (table_field_queries, index_queries) = parse_attributes(struct_fields, &table_name)?;

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

/// Get's the fields of the struct
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

/// Parses the `#[field]` and `#[index]` attributes on the fields of the struct
fn parse_attributes<'a>(
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

        // process the field attribute

        // what (if anything) should be appended to the plain field definition query
        let extra = match field_attrs.next() {
            Some(Ok((_, FieldAnnotation::Skip))) => {
                continue;
            }
            Some(Ok((_, FieldAnnotation::Plain))) => String::new(),
            Some(Ok((_, FieldAnnotation::Typed { type_ }))) => format!(" TYPE {}", type_.value()),
            Some(Ok((_, FieldAnnotation::CustomQuery { query }))) => {
                format!(" {}", query.value())
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

        table_field_queries.push(format!("DEFINE FIELD {field_name} ON {table_name}{extra};",));

        // next, we process the index attribute(s)
        let index_attrs = field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("index"))
            .map(|attr| {
                let parsed = attr.parse_args::<IndexAnnotation>();
                match parsed {
                    Ok(parsed) => Ok(parsed),
                    Err(err) => Err(err),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        for index in index_attrs {
            for query in index.to_query_strings(table_name, &field_name.to_string()) {
                index_queries.push(query);
            }
        }
    }

    Ok((table_field_queries, index_queries))
}

enum FieldAnnotation {
    Skip,
    Plain,
    Typed { type_: syn::LitStr },
    CustomQuery { query: syn::LitStr },
}

/// parses the `#[field]` attribute
///
/// the `#[field]` attribute can have the following keys:
/// - `skip`: if set, the field will be skipped
/// - `type`: the type of the field
impl Parse for FieldAnnotation {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let args: Punctuated<syn::Expr, syn::token::Comma> =
            input.parse_terminated(syn::Expr::parse, syn::token::Comma)?;

        if args.is_empty() {
            return Ok(Self::Plain);
        }

        if args.len() > 1 {
            return Err(syn::Error::new_spanned(
                args,
                "Field attribute can have at most one argument",
            ));
        }

        match args.first() {
            None => Ok(Self::Plain),
            Some(syn::Expr::Path(path)) if path.to_token_stream().to_string().eq("skip") => {
                Ok(Self::Skip)
            }
            Some(syn::Expr::Lit(ExprLit {
                lit: syn::Lit::Str(strlit),
                ..
            })) => Ok(Self::CustomQuery {
                query: strlit.clone(),
            }),
            Some(syn::Expr::Assign(ExprAssign { left, right, .. })) => {
                if left.to_token_stream().to_string().eq("dt") {
                    match *right.to_owned() {
                        syn::Expr::Lit(ExprLit {
                            lit: syn::Lit::Str(strlit),
                            ..
                        }) => Ok(Self::Typed { type_: strlit }),
                        _ => Err(syn::Error::new_spanned(
                            right,
                            "The `dt` attribute expects a string literal",
                        )),
                    }
                } else {
                    Err(syn::Error::new_spanned(
                        left,
                        "Unknown field attribute, expected `dt`",
                    ))
                }
            }
            Some(expr) => Err(syn::Error::new_spanned(
                expr,
                "Unsupported expression syntax, expected `skip`, `dt = \"type\"`, or a string literal representing a custom query",
            )),
        }
    }
}

#[derive(Default, Debug, Clone)]
struct IndexAnnotation {
    indexes: Vec<IndexAnnotationInner>,
}

#[derive(Debug, Clone)]
enum IndexAnnotationInner {
    Compound(CompoundIndexAnnotation),
    Single(IndexKind),
}

impl Parse for IndexAnnotation {
    /// Parses the `#[index]` attribute
    ///
    /// The syntax for compound attributes is:
    /// ```ignore
    /// #[index(compound(unique, "field1", "field2")]
    /// ```
    ///
    /// where `unique` is any of the valid index types, and `"field1"` and `"field2"` are the names of the fields to be indexed.
    /// - you can have more than 2 fields in a compound index, really any amount > 1.
    /// - the last arguments to `compound` must be strings representing field names the compound index is created on.
    /// - the first argument, if not a string must be a valid index type, or nothing to default to a normal index.
    ///
    /// Here are some representative examples of valid index types:
    /// ```ignore
    /// #[index(unique)]
    /// #[index()]
    /// #[index(vector(dim = 128))]
    /// #[index(text("english"))]
    /// #[index(compound(unique, "field1", "field2"))]
    /// #[index(compound("field1", "field2"))]
    /// ```
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // TODO: error if more than one of the same type of index is specified on the same field
        let args: Punctuated<syn::Expr, syn::token::Comma> =
            input.parse_terminated(syn::Expr::parse, syn::token::Comma)?;

        if args.is_empty() {
            return Ok(Self {
                indexes: vec![IndexAnnotationInner::Single(IndexKind::Normal)],
            });
        }

        let mut indexes = Vec::new();
        for arg in &args {
            match arg {
                syn::Expr::Call(call) if call.func.to_token_stream().to_string().eq("compound") => {
                    // parse the compound index from the
                    indexes.push(IndexAnnotationInner::Compound(
                        CompoundIndexAnnotation::parse(&call.args)?,
                    ));
                }
                _ => {
                    // parse the index type from the arg
                    let index_type = IndexKind::parse(Some(arg))?;
                    indexes.push(IndexAnnotationInner::Single(index_type));
                }
            }
        }

        Ok(Self { indexes })
    }
}

impl IndexAnnotation {
    // if both vector and full-text indexes are set, return None
    fn to_query_strings(&self, table_name: &str, field_name: &str) -> Vec<String> {
        let mut output = Vec::new();
        for index in &self.indexes {
            let (compound, index_type) = match index {
                IndexAnnotationInner::Compound(compound_index_annotation) => (
                    Some(&compound_index_annotation.fields),
                    &compound_index_annotation.index,
                ),
                IndexAnnotationInner::Single(index_kind) => (None, index_kind),
            };

            let (extra, index_type) = match index_type {
                IndexKind::Vector(vector) => (format!(" MTREE DIMENSION {}", vector.dim), "vector"),
                IndexKind::Text(text) => {
                    (format!(" SEARCH ANALYZER {} BM25", text.analyzer), "text")
                }
                IndexKind::Normal => (String::new(), "normal"),
                IndexKind::Unique => (String::from(" UNIQUE"), "unique"),
            };
            let compound_fields = |sep: &str| match compound {
                Some(compound) if !compound.is_empty() => {
                    format!("{sep}{}", compound.join(sep))
                }
                _ => String::new(),
            };

            let index_name = format!(
                "{table_name}_{field_name}{extra_fields}_{index_type}_index",
                extra_fields = compound_fields("_")
            );

            let query = format!(
                "DEFINE INDEX {index_name} ON {table_name} FIELDS {field_name}{extra_fields}{extra};",
                extra_fields = compound_fields(",")
            );

            output.push(query);
        }

        output
    }
}

#[derive(Default, Debug, Clone)]
/// A compound index is an index that is created across multiple fields.
struct CompoundIndexAnnotation {
    index: IndexKind,
    fields: Vec<String>,
}

impl CompoundIndexAnnotation {
    fn parse(args: &Punctuated<syn::Expr, syn::token::Comma>) -> syn::Result<Self> {
        let mut fields = Vec::new();

        let mut args_iter = args.iter();

        // the first argument (if not a string)
        let index = match args_iter.next() {
            Some(syn::Expr::Lit(ExprLit {
                lit: syn::Lit::Str(strlit),
                ..
            })) => {
                fields.push(strlit.value());
                IndexKind::Normal
            }
            arg => match IndexKind::parse(arg) {
                Ok(index_type) => index_type,
                Err(mut err) => {
                    err.combine(syn::Error::new_spanned(
                            arg,
                            "Compound index attribute expects a valid index type or string literal representing the first field name as the first argument",
                        ));
                    return Err(err);
                }
            },
        };

        // the remaining arguments should be string literals representing field names
        for arg in args_iter {
            match arg {
                syn::Expr::Lit(ExprLit {
                    lit: syn::Lit::Str(strlit),
                    ..
                }) => fields.push(strlit.value()),
                _ => {
                    return Err(syn::Error::new_spanned(
                        arg,
                        "Compound index attribute expects string literals representing the other field names",
                    ));
                }
            }
        }

        if fields.is_empty() {
            Err(syn::Error::new_spanned(
                args,
                "Compound index attribute expects at least one string literal representing the other field names",
            ))
        } else {
            Ok(Self { index, fields })
        }
    }
}

#[derive(Default, Debug, Clone)]
enum IndexKind {
    Vector(VectorIndexAnnotation),
    Text(TextIndexAnnotation),
    #[default]
    Normal,
    Unique,
}

impl IndexKind {
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
                        return Err(syn::Error::new_spanned(
                            right,
                            "`dim` expects an integer literal representing the number of dimensions in the vector",
                        ));
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
                ));
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
            _ => {
                return Err(syn::Error::new_spanned(
                    arg,
                    "Text index attribute expects a string literal representing the analyzer to use",
                ));
            }
        };

        Ok(Self { analyzer })
    }
}
