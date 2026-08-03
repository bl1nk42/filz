use colored::Colorize;
use comfy_table::{presets::UTF8_FULL, Attribute, Cell, Table};

pub fn format_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(
        headers
            .iter()
            .map(|h| Cell::new(*h).add_attribute(Attribute::Bold))
            .collect::<Vec<_>>(),
    );
    for row in rows {
        table.add_row(
            row.iter()
                .map(|c| Cell::new(c.as_str()))
                .collect::<Vec<_>>(),
        );
    }
    format!("{table}")
}

pub fn render_banner(title: &str) {
    let width = 60usize;
    let border = "═".repeat(width);
    let content = format!("{:^width$}", title).on_black().bold();
    println!("{}", border.on_black());
    println!("{}", content);
    println!("{}", border.on_black());
}

pub fn render_table(headers: &[&str], rows: &[Vec<String>]) {
    if rows.is_empty() {
        println!("{}", "ไม่มีข้อมูล".yellow());
        return;
    }
    println!("{}", format_table(headers, rows));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_table_uses_styled_borders() {
        let output = format_table(&["Name", "Count"], &[vec!["demo".into(), "1".into()]]);
        assert!(output.contains("┌") || output.contains("╭"));
        assert!(output.contains("demo"));
        assert!(output.contains("Count"));
    }
}
