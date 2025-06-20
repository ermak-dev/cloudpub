use std::path::Path;
use std::{env, fs};

fn main() {
    println!("cargo:rerun-if-changed=locales/");
    println!("cargo:rustc-check-cfg=cfg(locale, values(\"ru\", \"en\"))");

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("locale.ftl");

    // Get locale from environment variable, default to "ru"
    let locale = env::var("NEXT_PUBLIC_LANGUAGE").unwrap_or_else(|_| "ru".to_string());

    let locale_file = match locale.as_str() {
        "ru" => "locales/ru.ftl",
        "en" => "locales/en.ftl",
        _ => {
            println!(
                "cargo:warning=Unknown locale '{}', falling back to 'ru'",
                locale
            );
            "locales/ru.ftl"
        }
    };

    if Path::new(locale_file).exists() {
        fs::copy(locale_file, dest_path).unwrap();
    } else {
        panic!("Locale file {} not found", locale_file);
    }

    // Set a cfg flag for the locale
    println!("cargo:rustc-cfg=locale=\"{}\"", locale);
    println!("cargo:rerun-if-env-changed=NEXT_PUBLIC_LANGUAGE");
}
