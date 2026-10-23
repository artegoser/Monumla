use monumla_search::{SearchLimits, SearchOptions};
fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let game = args.next().unwrap_or_else(|| "chess".into());
    let depth = args
        .next()
        .map(|s| s.parse::<u16>().map_err(|e| e.to_string()))
        .transpose()?
        .unwrap_or(3);
    let nodes = args
        .next()
        .map(|s| s.parse::<u64>().map_err(|e| e.to_string()))
        .transpose()?
        .unwrap_or(10000);
    println!(
        "{:#?}",
        monumla_wasm::native_analysis(
            &game,
            SearchOptions {
                limits: SearchLimits {
                    max_depth: depth,
                    max_nodes: Some(nodes)
                },
                ..SearchOptions::default()
            }
        )?
    );
    Ok(())
}
