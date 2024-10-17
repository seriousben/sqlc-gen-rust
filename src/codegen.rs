use proc_macro2::TokenStream;
use quote::format_ident;
use quote::ToTokens;

use crate::ident;
use crate::plugin;

#[derive(Debug, Clone)]
enum Params {
    DBType(Vec<plugin::Column>),
    Struct { name: String, type_: String },
    None,
}

impl quote::ToTokens for Params {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Params::DBType(params) => {
                eprintln!("Params::ToTokens: {self:?}");
                let param_tokens = params.iter().map(|col| {
                    let field_name = format_ident!("{}", ident::to_snake(col.name.as_str()));
                    let field_type = convert_postgres_type(col);
                    quote::quote! { #field_name: #field_type }
                });
                quote::quote! { #(#param_tokens),* }.to_tokens(tokens);
            }
            Params::Struct { name, type_ } => {
                eprintln!("Params::ToTokens: {self:?}");
                let field_name = format_ident!("{}", ident::to_snake(name.as_str()));
                let field_type = format_ident!("{}", type_);
                quote::quote! { #field_name: #field_type }.to_tokens(tokens);
            }
            Params::None => {}
        }
    }
}

#[derive(Debug)]
struct GenField {
    col: plugin::Column,
}

impl quote::ToTokens for GenField {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        eprintln!("GenField::ToTokens: {self:?}");
        let field_name = format_ident!("{}", ident::to_snake(self.col.name.as_str()));
        let field_type = convert_postgres_type(&self.col);
        quote::quote! { #field_name: #field_type }.to_tokens(tokens);
    }
}

#[derive(Debug)]
pub struct GenStruct {
    name: String,
    fields: Vec<GenField>,
    cols: Vec<plugin::Column>,
}

impl GenStruct {}
impl quote::ToTokens for GenStruct {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        eprintln!("GenStruct::ToTokens: {self:?}");
        let fields = self.fields.as_slice();

        let struct_name = format_ident!("{}", self.name.as_str());
        quote::quote! {
            #[derive(Debug, Clone, sqlx::FromRow)]
            pub struct #struct_name {
                #(pub #fields),*
            }
        }
        .to_tokens(tokens);
    }
}

struct GenQuery<'query> {
    query: &'query plugin::Query,
    structs: Vec<&'query GenStruct>,
    params: Params,
    return_: TokenStream,
}

impl<'query> quote::ToTokens for GenQuery<'query> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        eprintln!("query::ToTokens: {:?}", self.query.name);
        let func_name = quote::format_ident!("{}", ident::to_snake(self.query.name.as_str()));
        let sql = format!("\n{}\n", self.query.text.as_str());

        let cmd = self.query.cmd.as_str();
        let params = self.params.clone();
        let structs_ = self.structs.as_slice();
        let return_tokens = self.return_.clone();

        let query_cols: Vec<plugin::Column> = self
            .query
            .columns
            .iter()
            .filter_map(|c| match c.r#type {
                Some(ref t) if t.name == "void" => None,
                _ => Some(c.clone()),
            })
            .collect();

        let params_bind_tokens = match &params {
            Params::DBType(params) => params
                .iter()
                .map(|col| {
                    let field_name = format_ident!("{}", ident::to_snake(col.name.as_str()));
                    quote::quote! { .bind(#field_name) }
                })
                .collect(),
            Params::Struct { name, type_ } => {
                let struct_name = format_ident!("{}", ident::to_snake(name));
                let struct_ = structs_
                    .iter()
                    .find(|struct_| struct_.name.as_str() == type_)
                    .expect("struct should exist");
                struct_
                    .fields
                    .iter()
                    .map(|field| {
                        let field_name =
                            format_ident!("{}", ident::to_snake(field.col.name.as_str()));
                        quote::quote! { .bind(#struct_name.#field_name) }.to_token_stream()
                    })
                    .collect()
            }
            Params::None => vec![],
        };

        let exec_func_tokens = match self.query.cmd.as_str() {
            ":one" => quote::quote! { fetch_one },
            ":many" => quote::quote! { fetch_all },
            ":exec" | ":execresult" | ":execrows" | ":copyfrom" => quote::quote! { execute },
            _ => panic!("unknown query command: {}", self.query.cmd),
        };

        let query_func_tokens = if query_cols.len() == 1 {
            quote::quote! { query_scalar }
        } else {
            match self.query.cmd.as_str() {
                ":one" | ":many" => quote::quote! { query_as },
                ":exec" | ":execresult" | ":execrows" | ":copyfrom" => quote::quote! { query },
                _ => panic!("unknown query command: {}", self.query.cmd),
            }
        };

        let fn_body_tokens = match self.query.cmd.as_str() {
            ":one" | ":many" => quote::quote! {
                let rec: #return_tokens = sqlx::#query_func_tokens(#sql)
                #(#params_bind_tokens)*
                .#exec_func_tokens(db)
                .await?;

                Ok(rec)
            },
            ":exec" => quote::quote! {
                sqlx::#query_func_tokens(#sql)
                #(#params_bind_tokens)*
                .#exec_func_tokens(db)
                .await?;

                Ok(())
            },
            ":execrows" => quote::quote! {
                let rec = sqlx::#query_func_tokens(#sql)
                #(#params_bind_tokens)*
                .#exec_func_tokens(db)
                .await?;

                Ok(rec.rows_affected())
            },
            ":execresult" | ":copyfrom" => quote::quote! {
                sqlx::#query_func_tokens(#sql)
                #(#params_bind_tokens)*
                .#exec_func_tokens(db)
                .await?;

                Err(sqlx::Error::TypeNotFound {
                    type_name: String::from(#cmd),
                })
            },
            _ => panic!("unknown query command: {}", self.query.cmd),
        };

        quote::quote! {
            #(#structs_)*

            pub async fn #func_name<'e, E>(db: E, #params) -> Result<#return_tokens, sqlx::Error>
            where
                E: sqlx::Executor<'e, Database = sqlx::Postgres>,
            {
                #fn_body_tokens
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
            let field = GenField { col: col.clone() };
            struct_.fields.push(field);
        }
        self.structs.push(Box::new(struct_));
        (self.structs.last().expect("last struct should exist"), true)
    }

    #[allow(clippy::too_many_lines)]
    fn gen_queries_file(&self) -> String {
        let queries = self.req.queries.iter().map(|query| {
            let mut new_structs = Vec::new();
            let params_cols: Vec<plugin::Column> = query
                .params
                .iter()
                .filter_map(|param| param.column.clone())
                .collect();
            let params = match params_cols.len() {
                3.. => {
                    let (info_struct, _) = self.find_or_create_struct(
                        format!("{}Info", query.name).as_str(),
                        params_cols.as_slice(),
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
                        .filter_map(|param| param.column.clone())
                        .collect(),
                ),
                0 => Params::None,
            };
            let query_cols: Vec<plugin::Column> = query
                .columns
                .iter()
                .filter_map(|c| match c.r#type {
                    Some(ref t) if t.name == "void" => None,
                    _ => Some(c.clone()),
                })
                .collect();
            let return_name = if query_cols.is_empty() {
                if query.cmd == ":execresult" {
                    panic!(":execresult not supported");
                } else if query.cmd == ":execrows" {
                    quote::quote! { u64 }
                } else {
                    quote::quote! { () }
                }
            } else if query_cols.len() == 1 {
                let col = query_cols.first().expect("first col should exist");
                convert_postgres_type(col)
            } else {
                let (ret_struct, new) = self.find_or_create_struct(
                    ident::to_upper_camel(format!("{}Row", query.name)).as_str(),
                    query_cols.as_slice(),
                );
                if new {
                    new_structs.push(ret_struct);
                }
                let ret_ident = format_ident!("{}", ret_struct.name.as_str());
                if query.cmd == ":many" {
                    let vec_ident = format_ident!("{}", "Vec");
                    quote::quote! { #vec_ident<#ret_ident> }
                } else {
                    ret_ident.to_token_stream()
                }
            };

            GenQuery {
                query,
                structs: new_structs,
                params,
                return_: return_name,
            }
        });
        let file = quote::quote! {
            /// This file is @generated by sqlc-gen-rust.
            #(#queries)*
        };
        pretty_print_ts(&file)
    }
}

#[allow(clippy::match_same_arms)]
fn convert_postgres_type(col: &plugin::Column) -> proc_macro2::TokenStream {
    let type_ = col
        .r#type
        .as_ref()
        .expect("col type expected")
        .name
        .as_str();
    let not_null = col.not_null;
    let is_array = col.is_array;
    let ident = match type_ {
        "serial" | "serial4" | "pg_catalog.serial4" => quote::quote! { i32 },
        "bigserial" | "serial8" | "pg_catalog.serial8" => quote::quote! { i64 },
        "smallserial" | "serial2" | "pg_catalog.serial2" => quote::quote! { i16 },
        "integer" | "int" | "int4" | "pg_catalog.int4" => quote::quote! { i32 },
        "bigint" | "int8" | "pg_catalog.int8" => quote::quote! { i64 },
        "smallint" | "int2" | "pg_catalog.int2" => quote::quote! { i16 },
        "float" | "double precision" | "float8" | "pg_catalog.float8" => quote::quote! { f64 },
        "real" | "float4" | "pg_catalog.float4" => quote::quote! { f32 },
        "numeric" | "pg_catalog.numeric" | "money" => quote::quote! { String },
        "boolean" | "bool" | "pg_catalog.bool" => quote::quote! { bool },
        "json" | "jsonb" => quote::quote! { sqlx::types::Json<serde_json::Value> },
        "bytea" | "blob" | "pg_catalog.bytea" => quote::quote! { Vec<u8> },
        "date" | "pg_catalog.date" => quote::quote! { chrono::NaiveDate },
        "time" | "pg_catalog.time" => quote::quote! { chrono::NaiveTime },
        "timestamp" | "pg_catalog.timestamp" => quote::quote! { chrono::NaiveDateTime },
        "timestamptz" | "pg_catalog.timestamptz" => quote::quote! { chrono::DateTime<chrono::Utc> },
        "text" | "pg_catalog.varchar" | "pg_catalog.bpchar" | "string" | "citext" | "name" => {
            quote::quote! { String }
        }
        "uuid" => quote::quote! { uuid::Uuid },
        "inet" | "cidr" => quote::quote! { std::net::IpAddr },
        "macaddr" | "macaddr8" => quote::quote! { eui48::MacAddress },
        "ltree" | "lquery" | "ltxtquery" => quote::quote! { String },
        "interval" | "pg_catalog.interval" => quote::quote! { chrono::Duration },
        _ => quote::quote! { sqlx::types::Json<serde_json::Value> },
    };

    if is_array {
        if not_null {
            quote::quote! { Vec<#ident> }
        } else {
            quote::quote! { Option<Vec<#ident>> }
        }
    } else if not_null {
        quote::quote! { #ident }
    } else {
        quote::quote! { Option<#ident> }
    }
}

fn pretty_print_ts(ts: &proc_macro2::TokenStream) -> String {
    let syn_file = syn::parse2::<syn::File>(ts.clone())
        .unwrap_or_else(|e| panic!("failed to parse tokens: {e:?}: \n{ts}\n"));
    let filestr = prettyplease::unparse(&syn_file);
    filestr
        .replace("\\n\")", "\\n\"\\n    )")
        .replace("\\n", "\n")
}
