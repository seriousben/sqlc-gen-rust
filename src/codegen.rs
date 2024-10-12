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
        quote::quote! { #field_name: #field_type }.to_tokens(tokens)
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
    return_: TokenStream,
}

impl<'query> GenQuery<'query> {}

impl<'query> quote::ToTokens for GenQuery<'query> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        eprintln!("query::ToTokens: {:?}", self.query.name);
        let func_name = quote::format_ident!("{}", ident::to_snake(self.query.name.as_str()));
        let sql = format!("\n{}\n", self.query.text.as_str());

        let params = self.params.clone();
        let structs_ = self.structs.as_slice();
        let return_tokens = self.return_.clone();

        quote::quote! {
            #(#structs_)*

            async fn #func_name(#params) -> Result<#return_tokens , Error> {
                let rec = sqlx::query!(
                    #sql
                );
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
                            let col = param.column.clone().expect("param.column expected");
                            Some(col)
                        })
                        .collect(),
                ),
                0 => Params::None,
            };
            let return_name = if query.columns.len() == 1 {
                let col = query.columns.first().expect("query.columns.first expected");
                if col.name.is_empty() {
                    convert_postgres_type(col)
                } else {
                    format_ident!("bool").to_token_stream()
                }
            } else {
                let (ret_struct, _) = self.find_or_create_struct(
                    format!("{}Row", query.name).as_str(),
                    query.columns.as_slice(),
                );
                new_structs.push(ret_struct);
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
        pretty_print_ts(file)
    }
}

fn format_ident_path(path: &[&str]) -> proc_macro2::TokenStream {
    let idents = path.iter().map(|s| format_ident!("{}", s));
    quote::quote! { #(#idents)::* }
}

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
        "serial" | "serial4" | "pg_catalog.serial4" => format_ident!("i32").to_token_stream(),
        "bigserial" | "serial8" | "pg_catalog.serial8" => format_ident!("i64").to_token_stream(),
        "smallserial" | "serial2" | "pg_catalog.serial2" => format_ident!("i16").to_token_stream(),
        "integer" | "int" | "int4" | "pg_catalog.int4" => format_ident!("i32").to_token_stream(),
        "bigint" | "int8" | "pg_catalog.int8" => format_ident!("i64").to_token_stream(),
        "smallint" | "int2" | "pg_catalog.int2" => format_ident!("i16").to_token_stream(),
        "float" | "double precision" | "float8" | "pg_catalog.float8" => {
            format_ident!("f64").to_token_stream()
        }
        "real" | "float4" | "pg_catalog.float4" => format_ident!("f32").to_token_stream(),
        "numeric" | "pg_catalog.numeric" | "money" => format_ident!("String").to_token_stream(),
        "boolean" | "bool" | "pg_catalog.bool" => format_ident!("bool").to_token_stream(),
        "json" | "jsonb" => format_ident_path(&["serde_json", "Value"]),
        "bytea" | "blob" | "pg_catalog.bytea" => format_ident!("u8").to_token_stream(),
        "date" | "pg_catalog.date" => format_ident_path(&["chrono", "NaiveDate"]),
        "time" | "pg_catalog.time" => format_ident_path(&["chrono", "NaiveTime"]),
        "timestamp" | "pg_catalog.timestamp" => format_ident_path(&["chrono", "NaiveDateTime"]),
        "timestamptz" | "pg_catalog.timestamptz" => {
            let chrono_ident = format_ident!("chrono");
            let datetime_ident = format_ident!("DateTime");
            let utc_ident = format_ident!("Utc");
            quote::quote! { #chrono_ident::#datetime_ident<#chrono_ident::#utc_ident> }
        }
        "text" | "pg_catalog.varchar" | "pg_catalog.bpchar" | "string" | "citext" | "name" => {
            format_ident!("String").to_token_stream()
        }
        "uuid" => format_ident_path(&["uuid", "Uuid"]),
        "inet" | "cidr" => format_ident_path(&["std", "net", "IpAddr"]),
        "macaddr" | "macaddr8" => format_ident_path(&["eui48", "MacAddress"]),
        "ltree" | "lquery" | "ltxtquery" => format_ident!("String").to_token_stream(),
        "interval" | "pg_catalog.interval" => format_ident_path(&["chrono", "Duration"]),
        _ => format_ident_path(&["serde_json", "Value"]),
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

fn pretty_print_ts(ts: proc_macro2::TokenStream) -> String {
    let syn_file = syn::parse2::<syn::File>(ts.clone())
        .unwrap_or_else(|e| panic!("failed to parse tokens: {e:?}: \n{ts:?}\n===\n{ts}"));
    let filestr = prettyplease::unparse(&syn_file);
    filestr.replace("\\n", "\n")
}
