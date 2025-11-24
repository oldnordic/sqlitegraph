use std::process::{Command, Stdio};

fn main() {
    let benches = ["bench_insert", "bench_traversal", "bench_algorithms"];
    println!("sqlitegraph bench driver\n========================");
    let results = collect_results(&benches, |bench| {
        Command::new("cargo")
            .args(["bench", "--bench", bench])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map(|code| code.success())
            .unwrap_or(false)
    });
    println!("\nSummary\n=======");
    let mut all_ok = true;
    for (bench, ok) in &results {
        println!("{bench:<20}{}", if *ok { "OK" } else { "FAIL" });
        all_ok &= *ok;
    }
    if !all_ok {
        std::process::exit(1);
    }
}

fn collect_results<'a, F>(benches: &[&'a str], runner: F) -> Vec<(&'a str, bool)>
where
    F: Fn(&str) -> bool,
{
    benches
        .iter()
        .map(|bench| (*bench, runner(bench)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::collect_results;

    #[test]
    fn test_collect_results_preserves_order_and_status() {
        let benches = ["a", "b", "c"];
        let results = collect_results(&benches, |name| name != "b");
        assert_eq!(results[0], ("a", true));
        assert_eq!(results[1], ("b", false));
        assert_eq!(results[2], ("c", true));
    }
}
