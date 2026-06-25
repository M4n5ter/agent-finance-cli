use serde_json::Value;

use crate::args::ResearchProvider;
use crate::model::{ResearchHighlight, research_value_string};

pub(super) fn quote_summary_root(payload: &Value) -> Option<&Value> {
    payload
        .pointer("/quoteSummary/result/0")
        .or_else(|| payload.pointer("/finance/result/0"))
}

pub(super) fn fundamentals_highlights(root: Option<&Value>) -> Vec<ResearchHighlight> {
    let mut rows = Vec::new();
    push_path(&mut rows, root, "Company", "/price/longName");
    push_path(&mut rows, root, "Exchange", "/price/exchangeName");
    push_path(&mut rows, root, "Industry", "/summaryProfile/industry");
    push_path(&mut rows, root, "Market cap", "/price/marketCap");
    push_path(
        &mut rows,
        root,
        "EV",
        "/defaultKeyStatistics/enterpriseValue",
    );
    push_path(&mut rows, root, "Trailing PE", "/summaryDetail/trailingPE");
    push_path(&mut rows, root, "Forward PE", "/summaryDetail/forwardPE");
    push_path(&mut rows, root, "P/B", "/defaultKeyStatistics/priceToBook");
    push_path(&mut rows, root, "Revenue", "/financialData/totalRevenue");
    push_path(
        &mut rows,
        root,
        "Revenue growth",
        "/financialData/revenueGrowth",
    );
    push_path(
        &mut rows,
        root,
        "Gross margin",
        "/financialData/grossMargins",
    );
    push_path(
        &mut rows,
        root,
        "Operating margin",
        "/financialData/operatingMargins",
    );
    push_path(
        &mut rows,
        root,
        "Free cash flow",
        "/financialData/freeCashflow",
    );
    push_path(&mut rows, root, "Cash", "/financialData/totalCash");
    push_path(&mut rows, root, "Debt", "/financialData/totalDebt");
    rows
}

pub(super) fn analysis_highlights(root: Option<&Value>) -> Vec<ResearchHighlight> {
    let mut rows = Vec::new();
    push_path(
        &mut rows,
        root,
        "Target mean price",
        "/financialData/targetMeanPrice",
    );
    push_path(
        &mut rows,
        root,
        "Target median price",
        "/financialData/targetMedianPrice",
    );
    push_path(
        &mut rows,
        root,
        "Target high price",
        "/financialData/targetHighPrice",
    );
    push_path(
        &mut rows,
        root,
        "Target low price",
        "/financialData/targetLowPrice",
    );
    push_path(
        &mut rows,
        root,
        "Recommendation key",
        "/financialData/recommendationKey",
    );
    push_path(
        &mut rows,
        root,
        "Analyst count",
        "/financialData/numberOfAnalystOpinions",
    );
    push_path(
        &mut rows,
        root,
        "Recommendation trend period",
        "/recommendationTrend/trend/0/period",
    );
    push_path(
        &mut rows,
        root,
        "Strong Buy",
        "/recommendationTrend/trend/0/strongBuy",
    );
    push_path(&mut rows, root, "Buy", "/recommendationTrend/trend/0/buy");
    push_path(&mut rows, root, "Hold", "/recommendationTrend/trend/0/hold");
    push_path(
        &mut rows,
        root,
        "EPS trend period",
        "/earningsTrend/trend/0/period",
    );
    push_path(
        &mut rows,
        root,
        "EPS estimate",
        "/earningsTrend/trend/0/earningsEstimate/avg",
    );
    push_path(
        &mut rows,
        root,
        "Revenue estimate",
        "/earningsTrend/trend/0/revenueEstimate/avg",
    );
    rows
}

pub(super) fn ownership_highlights(root: Option<&Value>) -> Vec<ResearchHighlight> {
    let mut rows = Vec::new();
    push_path(
        &mut rows,
        root,
        "Insider ownership",
        "/majorHoldersBreakdown/insidersPercentHeld",
    );
    push_path(
        &mut rows,
        root,
        "Institution ownership",
        "/majorHoldersBreakdown/institutionsPercentHeld",
    );
    push_path(
        &mut rows,
        root,
        "Institution float ownership",
        "/majorHoldersBreakdown/institutionsFloatPercentHeld",
    );
    push_path(
        &mut rows,
        root,
        "Institution count",
        "/majorHoldersBreakdown/institutionsCount",
    );
    push_path(
        &mut rows,
        root,
        "Insider transaction period",
        "/netSharePurchaseActivity/period",
    );
    push_path(
        &mut rows,
        root,
        "Insider net purchase",
        "/netSharePurchaseActivity/netPercentInsiderShares",
    );
    rows
}

pub(super) fn events_highlights(root: Option<&Value>) -> Vec<ResearchHighlight> {
    let mut rows = Vec::new();
    push_path(
        &mut rows,
        root,
        "Next earnings",
        "/calendarEvents/earnings/earningsDate/0",
    );
    push_path(
        &mut rows,
        root,
        "EPS mean",
        "/calendarEvents/earnings/earningsAverage",
    );
    push_path(
        &mut rows,
        root,
        "Revenue mean",
        "/calendarEvents/earnings/revenueAverage",
    );
    push_path(
        &mut rows,
        root,
        "Ex-dividend date",
        "/summaryDetail/exDividendDate",
    );
    push_path(
        &mut rows,
        root,
        "Dividend yield",
        "/summaryDetail/dividendYield",
    );
    push_path(&mut rows, root, "SEC filing", "/secFilings/filings/0/type");
    push_path(&mut rows, root, "SEC date", "/secFilings/filings/0/date");
    push_path(
        &mut rows,
        root,
        "SEC link",
        "/secFilings/filings/0/edgarUrl",
    );
    rows
}

pub(super) fn options_highlights(payload: &Value) -> Vec<ResearchHighlight> {
    let result = payload.pointer("/optionChain/result/0");
    let mut rows = Vec::new();
    push_yahoo_path(
        &mut rows,
        result,
        "Underlying price",
        "/quote/regularMarketPrice",
        "options",
    );
    push_yahoo_path(
        &mut rows,
        result,
        "Available expiries",
        "/expirationDates",
        "options",
    );
    if let Some(option) = result.and_then(|root| root.pointer("/options/0")) {
        rows.push(ResearchHighlight::new(
            "Call contracts",
            option
                .pointer("/calls")
                .and_then(Value::as_array)
                .map(|values| values.len().to_string())
                .unwrap_or_else(|| "-".to_string()),
            ResearchProvider::Yahoo.label(),
            "options",
        ));
        rows.push(ResearchHighlight::new(
            "Put contracts",
            option
                .pointer("/puts")
                .and_then(Value::as_array)
                .map(|values| values.len().to_string())
                .unwrap_or_else(|| "-".to_string()),
            ResearchProvider::Yahoo.label(),
            "options",
        ));
    }
    rows
}

pub(super) fn search_highlights(payload: &Value) -> Vec<ResearchHighlight> {
    let mut rows = Vec::new();
    if let Some(quotes) = payload.pointer("/quotes").and_then(Value::as_array) {
        for quote in quotes.iter().take(8) {
            let symbol =
                research_value_string(quote.pointer("/symbol")).unwrap_or_else(|| "-".to_string());
            let name = research_value_string(quote.pointer("/shortname"))
                .or_else(|| research_value_string(quote.pointer("/longname")))
                .unwrap_or_else(|| "-".to_string());
            rows.push(ResearchHighlight::new(
                &format!("Ticker {symbol}"),
                name,
                ResearchProvider::Yahoo.label(),
                "search",
            ));
        }
    }
    if let Some(news) = payload.pointer("/news").and_then(Value::as_array) {
        for item in news.iter().take(5) {
            let title =
                research_value_string(item.pointer("/title")).unwrap_or_else(|| "-".to_string());
            let publisher = research_value_string(item.pointer("/publisher"))
                .unwrap_or_else(|| "-".to_string());
            rows.push(ResearchHighlight::new(
                &format!("News {publisher}"),
                title,
                ResearchProvider::Yahoo.label(),
                "news",
            ));
        }
    }
    rows
}

pub(super) fn screen_highlights(payload: &Value) -> Vec<ResearchHighlight> {
    let quotes = payload
        .pointer("/finance/result/0/quotes")
        .and_then(Value::as_array)
        .or_else(|| payload.pointer("/quotes").and_then(Value::as_array));
    let mut rows = Vec::new();
    if let Some(quotes) = quotes {
        for quote in quotes.iter().take(25) {
            let symbol =
                research_value_string(quote.pointer("/symbol")).unwrap_or_else(|| "-".to_string());
            let name = research_value_string(quote.pointer("/shortName"))
                .or_else(|| research_value_string(quote.pointer("/longName")))
                .unwrap_or_else(|| "-".to_string());
            let price = research_value_string(quote.pointer("/regularMarketPrice"))
                .or_else(|| research_value_string(quote.pointer("/regularMarketPrice/fmt")))
                .unwrap_or_else(|| "-".to_string());
            rows.push(ResearchHighlight::new(
                &symbol,
                format!("{name} | {price}"),
                ResearchProvider::Yahoo.label(),
                "screen",
            ));
        }
    }
    rows
}

pub(super) fn push_path(
    rows: &mut Vec<ResearchHighlight>,
    root: Option<&Value>,
    label: &str,
    path: &str,
) {
    push_yahoo_path(rows, root, label, path, "quoteSummary");
}

pub(super) fn push_yahoo_path(
    rows: &mut Vec<ResearchHighlight>,
    root: Option<&Value>,
    label: &str,
    path: &str,
    module: &str,
) {
    let Some(root) = root else {
        return;
    };
    if let Some(row) = ResearchHighlight::from_path(
        Some(root),
        label,
        path,
        ResearchProvider::Yahoo.label(),
        module,
    ) {
        rows.push(row);
    }
}
