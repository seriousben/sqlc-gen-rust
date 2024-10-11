use quote::format_ident;

use crate::ident;
use crate::plugin;

#[derive(Debug, Clone)]
enum Params {
    DBType(Vec<(String, String, bool)>),
    Struct { name: String, type_: String },
    None,
}

impl quote::ToTokens for Params {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Params::DBType(params) => {
                let param_tokens = params.iter().map(|(name, type_, not_null)| {
                    let field_name = format_ident!("{}", ident::to_snake(name.as_str()));
                    let field_type = convert_postgres_type(type_.as_str(), *not_null);
                    quote::quote! { #field_name: #field_type }
                });
                quote::quote! { #(#param_tokens),* }.to_tokens(tokens);
            }
            Params::Struct { name, type_ } => {
                let field_name = format_ident!("{}", ident::to_snake(name.as_str()));
                quote::quote! { #field_name: #type_ }.to_tokens(tokens);
            }
            Params::None => {}
        }
    }
}

struct GenField {
    name: String,
    type_: String,
    struct_type: String,
    not_null: bool,
}

impl quote::ToTokens for GenField {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let field_name = format_ident!("{}", ident::to_snake(self.name.as_str()));
        let field_type = if self.struct_type.is_empty() {
            convert_postgres_type(self.type_.as_str(), self.not_null)
        } else {
            quote::quote! { self.struct_type }
        };
        quote::quote! { #field_name: #field_type }.to_tokens(tokens);
    }
}

pub struct GenStruct {
    name: String,
    fields: Vec<GenField>,
    cols: Vec<plugin::Column>,
}

impl GenStruct {}
impl quote::ToTokens for GenStruct {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let fields = self.fields.as_slice();

        let struct_name = format_ident!("{}", ident::to_upper_camel(self.name.as_str()));
        quote::quote! {
            #[derive(Debug, Clone)]
            pub struct #struct_name {
                #(#fields),*
            }
        }
        .to_tokens(tokens);
    }
}

struct GenQuery<'query> {
    query: &'query plugin::Query,
    structs: Vec<&'query GenStruct>,
    params: Params,
    return_: String,
}

impl<'query> GenQuery<'query> {}

impl<'query> quote::ToTokens for GenQuery<'query> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let func_name = quote::format_ident!("{}", ident::to_snake(self.query.name.as_str()));
        let sql = self.query.text.replace('\n', " ").replace("  ", " ");

        let params = self.params.clone();
        let structs_ = self.structs.as_slice();
        let return_tokens = format_ident!("{}", self.return_.as_str());

        quote::quote! {
            #(#structs_)*

            async fn #func_name(#params) -> Result<#return_tokens , Error> {
                let rec = sqlx::query!(#sql);
            }
        }
        .to_tokens(tokens);
    }
}

pub struct Generator {
    pub req: plugin::GenerateRequest,
    pub structs: elsa::vec::FrozenVec<Box<GenStruct>>,
}

impl Generator {
    pub fn generate(&mut self) -> Vec<plugin::File> {
        let queries = self.gen_queries_file();

        vec![plugin::File {
            name: "queries.rs".to_string(),
            contents: queries.as_bytes().to_vec(),
        }]
    }

    fn struct_find(&self, name: &str, cols: &[plugin::Column]) -> Option<&GenStruct> {
        self.structs.iter().find(|struct_| {
            struct_.name.as_str() == name
                && struct_
                    .cols
                    .iter()
                    .all(|field| cols.iter().any(|col| col == field))
        })
    }

    fn struct_exists(&self, name: &str, cols: &[plugin::Column]) -> bool {
        self.struct_find(name, cols).is_some()
    }

    fn find_or_create_struct(&self, name: &str, cols: &[plugin::Column]) -> (&GenStruct, bool) {
        if self.struct_exists(name, cols) {
            return (
                self.struct_find(name, cols)
                    .expect("existing struct should be found"),
                false,
            );
        }
        let mut struct_ = GenStruct {
            name: String::from(name),
            fields: Vec::new(),
            cols: cols.to_vec(),
        };
        for col in cols {
            let field = GenField {
                name: ident::to_upper_camel(col.name.as_str()),
                type_: col
                    .r#type
                    .as_ref()
                    .expect("col.type expected")
                    .name
                    .as_str()
                    .to_string(),
                not_null: col.not_null,
                struct_type: String::new(),
            };
            struct_.fields.push(field);
        }
        self.structs.push(Box::new(struct_));
        (self.structs.last().expect("last struct should exist"), true)
    }

    fn gen_queries_file(&self) -> String {
        let queries = self.req.queries.iter().map(|query| {
            let mut new_structs = Vec::new();
            let cols: Vec<plugin::Column> = query
                .params
                .iter()
                .filter_map(|param| param.column.clone())
                .collect();
            let params = match cols.len() {
                3.. => {
                    let (info_struct, _) = self.find_or_create_struct(
                        format!("{}Info", query.name).as_str(),
                        cols.as_slice(),
                    );
                    let struct_ = Params::Struct {
                        name: ident::to_snake(info_struct.name.as_str()),
                        type_: info_struct.name.clone(),
                    };
                    new_structs.push(info_struct);
                    struct_
                }
                1..3 => Params::DBType(
                    query
                        .params
                        .iter()
                        .filter_map(|param| {
                            if param.column.is_none() {
                                return None;
                            }
                            let col = param.column.as_ref().expect("param.column expected");
                            Some((
                                ident::to_snake(col.name.as_str()),
                                String::from(
                                    col.r#type
                                        .as_ref()
                                        .expect("col.type expected")
                                        .name
                                        .as_str(),
                                ),
                                col.not_null,
                            ))
                        })
                        .collect(),
                ),
                0 => Params::None,
            };
            let (ret_struct, _) = self.find_or_create_struct(
                format!("{}Row", query.name).as_str(),
                query.columns.as_slice(),
            );
            new_structs.push(ret_struct);

            GenQuery {
                query,
                structs: new_structs,
                params,
                return_: ret_struct.name.clone(),
            }
        });
        let file = quote::quote! {
            /// This file is @generated by sqlc-gen-rust.
            #(#queries)*
        };
        pretty_print_ts(file)
    }
}

fn convert_postgres_type(type_: &str, not_null: bool) -> proc_macro2::TokenStream {
    match type_ {
        "serial" | "serial4" | "pg_catalog.serial4" => {
            if not_null {
                quote::quote! { i32 }
            } else {
                quote::quote! { Option<i32> }
            }
        }
        "bigserial" | "serial8" | "pg_catalog.serial8" => {
            if not_null {
                quote::quote! { i64 }
            } else {
                quote::quote! { Option<i64> }
            }
        }
        "smallserial" | "serial2" | "pg_catalog.serial2" => {
            if not_null {
                quote::quote! { i16 }
            } else {
                quote::quote! { Option<i16> }
            }
        }
        "integer" | "int" | "int4" | "pg_catalog.int4" => {
            if not_null {
                quote::quote! { i32 }
            } else {
                quote::quote! { Option<i32> }
            }
        }
        "bigint" | "int8" | "pg_catalog.int8" => {
            if not_null {
                quote::quote! { i64 }
            } else {
                quote::quote! { Option<i64> }
            }
        }
        "smallint" | "int2" | "pg_catalog.int2" => {
            if not_null {
                quote::quote! { i16 }
            } else {
                quote::quote! { Option<i16> }
            }
        }
        "float" | "double precision" | "float8" | "pg_catalog.float8" => {
            if not_null {
                quote::quote! { f64 }
            } else {
                quote::quote! { Option<f64> }
            }
        }
        "real" | "float4" | "pg_catalog.float4" => {
            if not_null {
                quote::quote! { f32 }
            } else {
                quote::quote! { Option<f32> }
            }
        }
        "numeric" | "pg_catalog.numeric" | "money" => {
            if not_null {
                quote::quote! { String }
            } else {
                quote::quote! { Option<String> }
            }
        }
        "boolean" | "bool" | "pg_catalog.bool" => {
            if not_null {
                quote::quote! { bool }
            } else {
                quote::quote! { Option<bool> }
            }
        }
        "json" | "jsonb" => {
            if not_null {
                quote::quote! { serde_json::Value }
            } else {
                quote::quote! { Option<serde_json::Value> }
            }
        }
        "bytea" | "blob" | "pg_catalog.bytea" => quote::quote! { Vec<u8> },
        "date" | "pg_catalog.date" => {
            if not_null {
                quote::quote! { chrono::NaiveDate }
            } else {
                quote::quote! { Option<chrono::NaiveDate> }
            }
        }
        "time" | "pg_catalog.time" => {
            if not_null {
                quote::quote! { chrono::NaiveTime }
            } else {
                quote::quote! { Option<chrono::NaiveTime> }
            }
        }
        "timestamp" | "pg_catalog.timestamp" => {
            if not_null {
                quote::quote! { chrono::NaiveDateTime }
            } else {
                quote::quote! { Option<chrono::NaiveDateTime> }
            }
        }
        "timestamptz" | "pg_catalog.timestamptz" => {
            if not_null {
                quote::quote! { chrono::DateTime<chrono::Utc> }
            } else {
                quote::quote! { Option<chrono::DateTime<chrono::Utc>> }
            }
        }
        "text" | "pg_catalog.varchar" | "pg_catalog.bpchar" | "string" | "citext" | "name" => {
            if not_null {
                quote::quote! { String }
            } else {
                quote::quote! { Option<String> }
            }
        }
        "uuid" => {
            if not_null {
                quote::quote! { uuid::Uuid }
            } else {
                quote::quote! { Option<uuid::Uuid> }
            }
        }
        "inet" => {
            if not_null {
                quote::quote! { std::net::IpAddr }
            } else {
                quote::quote! { Option<std::net::IpAddr> }
            }
        }
        "cidr" => {
            if not_null {
                quote::quote! { std::net::IpAddr }
            } else {
                quote::quote! { Option<std::net::IpAddr> }
            }
        }
        "macaddr" | "macaddr8" => {
            if not_null {
                quote::quote! { eui48::MacAddress }
            } else {
                quote::quote! { Option<eui48::MacAddress> }
            }
        }
        "ltree" | "lquery" | "ltxtquery" => {
            if not_null {
                quote::quote! { String }
            } else {
                quote::quote! { Option<String> }
            }
        }
        "interval" | "pg_catalog.interval" => {
            if not_null {
                quote::quote! { chrono::Duration }
            } else {
                quote::quote! { Option<chrono::Duration> }
            }
        }
        _ => quote::quote! { Option<serde_json::Value> },
    }
}

fn pretty_print_ts(ts: proc_macro2::TokenStream) -> String {
    let syn_file = syn::parse2::<syn::File>(ts.clone())
        .unwrap_or_else(|e| panic!("failed to parse tokens: {}: \n{}", e, ts,));
    prettyplease::unparse(&syn_file)
}
