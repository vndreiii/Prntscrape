use chrono::Local;
use std::path::PathBuf;

pub fn generate_filepath(
    save_dir: &str,
    app_class: &str,
    project: Option<&str>,
    ext: &str,
) -> PathBuf {
    let now = Local::now();
    let date_str = now.format("%d-%m-%Y").to_string();
    let time_str = now.format("%H%M%S").to_string();

    let app_slug = slugify(app_class);

    let mut path = PathBuf::from(save_dir);

    match project.filter(|p| !p.trim().is_empty()) {
        Some(proj) => {
            let proj_slug = slugify(proj);
            path.push(&proj_slug);
            path.push(&date_str);
            let filename = format!(
                "{}-{}-{}-{}.{}",
                proj_slug, app_slug, date_str, time_str, ext
            );
            path.push(filename);
        }
        None => {
            path.push(&app_slug);
            path.push(&date_str);
            let filename = format!("{}-{}-{}.{}", app_slug, date_str, time_str, ext);
            path.push(filename);
        }
    }

    path
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || c.is_ascii_whitespace())
        .map(|c| if c.is_ascii_whitespace() { '-' } else { c })
        .collect::<String>()
}
