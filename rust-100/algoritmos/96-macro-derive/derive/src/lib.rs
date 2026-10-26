//! # `#[derive(Describir)]`
//!
//! Una macro *derive* recibe el código del tipo anotado como `TokenStream`.
//! La parseamos con `syn` y devolvemos **código nuevo**: un `impl Describir`.
//! Ese código se añade junto al tipo original, que no se modifica.
//!
//! Atributos que soporta:
//!
//! - A nivel de tipo: `#[describir(nombre = "Otro")]` cambia el nombre que se muestra.
//! - A nivel de campo o de variante:
//!   - `#[describir(omitir)]` lo oculta, útil para contraseñas.
//!   - `#[describir(renombrar = "x")]` cambia su etiqueta.
//!
//! Los errores se devuelven con `syn::Error::to_compile_error()`, apuntando
//! al *span* exacto del problema, como haría el compilador.
#![deny(missing_docs)]

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TS2;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, parse_quote, Attribute, Data, DeriveInput, Fields, GenericParam, LitStr,
};

/// Punto de entrada. `attributes(describir)` registra `#[describir(...)]`
/// como atributo "inerte" para que el compilador no lo rechace.
#[proc_macro_derive(Describir, attributes(describir))]
pub fn derive_describir(entrada: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(entrada as DeriveInput);
    match expandir(ast) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Opciones leídas de los atributos `#[describir(...)]` de un elemento.
#[derive(Default)]
struct Opciones {
    omitir: bool,
    renombrar: Option<String>,
    nombre: Option<String>,
}

/// Recorre los atributos y extrae nuestras opciones. Usa la API
/// `parse_nested_meta` de syn 2, que valida la sintaxis `clave` / `clave = "valor"`.
fn leer_opciones(attrs: &[Attribute], es_tipo: bool) -> syn::Result<Opciones> {
    let mut op = Opciones::default();
    for attr in attrs.iter().filter(|a| a.path().is_ident("describir")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("omitir") && !es_tipo {
                op.omitir = true;
                Ok(())
            } else if meta.path.is_ident("renombrar") && !es_tipo {
                op.renombrar = Some(meta.value()?.parse::<LitStr>()?.value());
                Ok(())
            } else if meta.path.is_ident("nombre") && es_tipo {
                op.nombre = Some(meta.value()?.parse::<LitStr>()?.value());
                Ok(())
            } else {
                // `meta.error` señala el identificador desconocido.
                Err(meta.error(if es_tipo {
                    "atributo de tipo desconocido; se admite: nombre = \"...\""
                } else {
                    "atributo de campo desconocido; se admite: omitir, renombrar = \"...\""
                }))
            }
        })?;
    }
    Ok(op)
}

/// Genera el código que describe un conjunto de campos ya "desestructurados"
/// en variables `__c0, __c1, ...`. Devuelve:
/// - el patrón de desestructuración (`{ a: __c0, .. }`, `(__c0, _)` o vacío);
/// - el código que escribe los campos en `__s`;
/// - la lista de etiquetas visibles (para `campos()`).
fn describir_campos(campos: &Fields) -> syn::Result<(TS2, TS2, Vec<String>)> {
    let mut patrones = Vec::new();
    let mut escrituras = Vec::new();
    let mut etiquetas = Vec::new();
    for (i, campo) in campos.iter().enumerate() {
        let op = leer_opciones(&campo.attrs, false)?;
        let var = format_ident!("__c{}", i);
        // Patrón: con nombre `ident: var`, o posicional `var`; si se omite, `_`.
        let enlace = if op.omitir { quote!(_) } else { quote!(#var) };
        match &campo.ident {
            Some(id) => patrones.push(quote!(#id: #enlace)),
            None => patrones.push(enlace),
        }
        if op.omitir {
            continue;
        }
        let etiqueta = op
            .renombrar
            .or_else(|| campo.ident.as_ref().map(|id| id.to_string()))
            .unwrap_or_else(|| i.to_string());
        let sep = if etiquetas.is_empty() { "" } else { ", " };
        // Los campos con nombre se muestran como `etq: valor`; los
        // posicionales solo como valor (salvo que se renombren).
        let es_nombrado = campo.ident.is_some() || etiqueta.parse::<usize>().is_err();
        let prefijo = if es_nombrado { format!("{sep}{etiqueta}: ") } else { sep.to_string() };
        escrituras.push(quote! {
            __s.push_str(#prefijo);
            __s.push_str(&::std::format!("{:?}", #var));
        });
        etiquetas.push(etiqueta);
    }
    let (abre, cierra) = match campos {
        Fields::Named(_) => (" { ", " }"),
        Fields::Unnamed(_) => ("(", ")"),
        Fields::Unit => ("", ""),
    };
    // Si hay campos omitidos, mostramos `..` para que se note que hay más.
    let hay_omitidos = etiquetas.len() < campos.len();
    let resto = match (hay_omitidos, etiquetas.is_empty()) {
        (true, true) => "..",
        (true, false) => ", ..",
        _ => "",
    };
    let patron = match campos {
        Fields::Named(_) => quote!({ #(#patrones),* }),
        Fields::Unnamed(_) => quote!(( #(#patrones),* )),
        Fields::Unit => quote!(),
    };
    let cuerpo = if matches!(campos, Fields::Unit) {
        quote!()
    } else {
        quote! {
            __s.push_str(#abre);
            #(#escrituras)*
            __s.push_str(#resto);
            __s.push_str(#cierra);
        }
    };
    Ok((patron, cuerpo, etiquetas))
}

/// Construye el `impl` completo.
fn expandir(mut ast: DeriveInput) -> syn::Result<TS2> {
    let op_tipo = leer_opciones(&ast.attrs, true)?;
    let ident = &ast.ident;
    let nombre = op_tipo.nombre.unwrap_or_else(|| ident.to_string());

    // Genéricos: cada parámetro de tipo T debe implementar Debug para que
    // `format!("{:?}")` funcione con él. Añadimos ese *bound* automáticamente.
    for p in ast.generics.params.iter_mut() {
        if let GenericParam::Type(t) = p {
            t.bounds.push(parse_quote!(::std::fmt::Debug));
        }
    }
    let (impl_g, ty_g, where_g) = ast.generics.split_for_impl();

    let (cuerpo, campos_lista) = match &ast.data {
        Data::Struct(s) => {
            let (patron, escribir, etiquetas) = describir_campos(&s.fields)?;
            (
                quote! {
                    let #ident #patron = self;
                    __s.push_str(#nombre);
                    #escribir
                },
                etiquetas,
            )
        }
        Data::Enum(e) => {
            if e.variants.is_empty() {
                return Err(syn::Error::new_spanned(ident, "Describir no admite enums sin variantes"));
            }
            let mut brazos = Vec::new();
            let mut nombres_variantes = Vec::new();
            for v in &e.variants {
                let op_v = leer_opciones(&v.attrs, false)?;
                if op_v.omitir {
                    return Err(syn::Error::new_spanned(v, "no se puede omitir una variante"));
                }
                let vid = &v.ident;
                let etiqueta = format!("{nombre}::{}", op_v.renombrar.unwrap_or_else(|| vid.to_string()));
                let (patron, escribir, _) = describir_campos(&v.fields)?;
                brazos.push(quote! {
                    #ident::#vid #patron => {
                        __s.push_str(#etiqueta);
                        #escribir
                    }
                });
                nombres_variantes.push(vid.to_string());
            }
            (quote! { match self { #(#brazos)* } }, nombres_variantes)
        }
        Data::Union(u) => {
            return Err(syn::Error::new_spanned(
                u.union_token,
                "Describir no admite `union`: no se sabe qué campo está activo",
            ))
        }
    };

    // `::describir::` con ruta absoluta: el código generado funciona aunque
    // el usuario tenga otro módulo llamado igual. Dentro del propio crate
    // `describir` se resuelve gracias a `extern crate self as describir;`.
    Ok(quote! {
        #[automatically_derived]
        impl #impl_g ::describir::Describir for #ident #ty_g #where_g {
            fn nombre_tipo() -> &'static str { #nombre }
            fn campos() -> &'static [&'static str] { &[#(#campos_lista),*] }
            fn describir(&self) -> ::std::string::String {
                let mut __s = ::std::string::String::new();
                #cuerpo
                __s
            }
        }
    })
}
