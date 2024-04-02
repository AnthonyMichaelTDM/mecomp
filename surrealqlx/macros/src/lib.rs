use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse::Parse, parse_macro_input, Data, DeriveInput}; // Add this import

/// I want to make a derive macro that will implement the Table trait for a struct
/// with the given table_name.
///
/// the name of the table will be given by an attribute #[Table("table_name")]
///
/// the table trait defines a const TABLE_NAME: &'static str, and a fn init_table() -> String
/// the function returns a string that is a SurrealQL query that creates the table
///
/// each field in the struct will be a field in the table, which will be created with the same name, and the type given by the `#[field]` attribute.
///
/// if a field is missing the `#[field]` attribute, the macro will return an error, unless it is annotated #[field(ignore)] and is either an `Option`, or has a default value.
///
/// the macro will also implement the Table trait for the struct

struct FieldAnnotation {
    skip: bool,
    type_: Option<syn::LitStr>,
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

        while !input.is_empty() {
            // it is either the ident `skip`, or a lit string that is the type
            let ident: syn::Ident = input.parse()?;

            match ident.to_string().as_str() {
                "skip" => {
                    skip = true;
                }
                "dt" => {
                    input.parse::<syn::Token![=]>()?;
                    let lit: syn::LitStr = input.parse()?;
                    type_ = Some(lit);
                }
                _ => {
                    // if it is neither `skip` nor `type`, try to parse a litstr as the type or return an error
                    return Err(syn::Error::new_spanned(
                        ident,
                        "Unknown field attribute, expected `skip` or `type`",
                    ));
                }
            }
        }

        Ok(FieldAnnotation { skip, type_ })
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
        .and_then(|attr| {
            attr.parse_args::<syn::LitStr>()
                .map(|lit| lit.value())
                .map_err(|err| err)
        })?;
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
) -> syn::Result<String> {
    let mut table_field_queries = String::new();

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
        let field_type = match field_attrs.next() {
            Some(Ok((_, FieldAnnotation { skip: true, .. }))) => {
                continue;
            }
            Some(Ok((
                _,
                FieldAnnotation {
                    skip: false,
                    type_: Some(type_),
                },
            ))) => type_.value(),
            Some(Ok((
                field_attr,
                FieldAnnotation {
                    skip: false,
                    type_: None,
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

        table_field_queries.push_str(&format!(
            "DEFINE FIELD IF NOT EXISTS {} ON {} TYPE {};\n",
            field_name, table_name, field_type
        ));
    }

    Ok(table_field_queries)
}

#[proc_macro_derive(Table, attributes(Table, field))]
pub fn table_macro(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input as DeriveInput);

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

    let table_field_queries = match create_table_field_queries(struct_fields, &table_name) {
        Ok(queries) => queries,
        Err(err) => {
            return err.to_compile_error().into();
        }
    };

    let surrealql_query = format!(
        "BEGIN TRANSACTION;\n DEFINE TABLE IF NOT EXISTS {} SCHEMAFULL;\n{}\nCOMMIT TRANSACTION;",
        table_name, table_field_queries
    )
    .into_token_stream();

    // Build the output, possibly using the input
    let expanded = quote! {
        // The generated impl goes here
        impl ::surrealqlx::traits::Table for #struct_name {
            const TABLE_NAME: &'static str = #table_name;
            const TABLE_SCHEMA_QUERY: &'static str = #surrealql_query;
        }
    };

    // Hand the output tokens back to the compiler
    expanded.into()
}
