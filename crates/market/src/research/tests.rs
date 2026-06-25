use super::*;
use serde_json::json;

#[test]
fn fundamentals_highlights_extract_raw_and_fmt_values() {
    let payload = json!({
        "quoteSummary": {
            "result": [{
                "price": {
                    "longName": "Credo Technology Group Holding Ltd",
                    "marketCap": {"raw": 41704132608_i64, "fmt": "41.7B"}
                },
                "summaryProfile": {"industry": "Semiconductors"},
                "financialData": {
                    "revenueGrowth": {"raw": 2.015, "fmt": "201.5%"},
                    "freeCashflow": {"raw": 172241120_i64}
                }
            }]
        }
    });
    let highlights = fundamentals_highlights(quote_summary_root(&payload));
    assert!(
        highlights
            .iter()
            .any(|row| row.label == "Company" && row.value == "Credo Technology Group Holding Ltd")
    );
    assert!(
        highlights
            .iter()
            .any(|row| row.label == "Market cap" && row.value == "41.7B")
    );
    assert!(
        highlights
            .iter()
            .any(|row| row.label == "Revenue growth" && row.value == "201.5%")
    );
    assert!(
        highlights
            .iter()
            .any(|row| row.label == "Free cash flow" && row.value == "172241120")
    );
}
