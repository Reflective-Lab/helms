use std::fs::{self, File};
use std::path::Path;

use anyhow::Result;
use chrono::{NaiveDate, TimeDelta};
use polars::prelude::*;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

const SEED: u64 = 42;
const OUTPUT_DIR: &str = "data/seed";

fn main() -> Result<()> {
    fs::create_dir_all(OUTPUT_DIR)?;

    let mut rng = SmallRng::seed_from_u64(SEED);

    let prospects = prospect_profiles();
    let behavior_df = generate_behavior_events(&prospects, &mut rng)?;
    let account_df = generate_account_context(&prospects, &mut rng)?;
    let calendar_df = generate_calendar_availability(&mut rng)?;

    write_parquet(
        &behavior_df,
        &Path::new(OUTPUT_DIR).join("behavior_events.parquet"),
    )?;
    write_parquet(
        &account_df,
        &Path::new(OUTPUT_DIR).join("account_context.parquet"),
    )?;
    write_parquet(
        &calendar_df,
        &Path::new(OUTPUT_DIR).join("calendar_availability.parquet"),
    )?;

    let rows = behavior_df.height();
    let prospects_n = prospects.len();
    println!("seed data written to {OUTPUT_DIR}/");
    println!("  behavior_events.parquet   — {rows} rows across {prospects_n} prospects");
    println!("  account_context.parquet   — {prospects_n} rows");
    println!(
        "  calendar_availability.parquet — {} slots",
        calendar_df.height()
    );

    Ok(())
}

struct ProspectProfile {
    id: &'static str,
    name: &'static str,
    persona: Persona,
    industry: &'static str,
    employee_count: u32,
    funding_stage: &'static str,
}

#[derive(Clone, Copy)]
enum Persona {
    HighIntent,
    DeepTechnical,
    TireKicker,
    EnterpriseEvaluator,
    QuickWin,
    Dormant,
}

fn prospect_profiles() -> Vec<ProspectProfile> {
    vec![
        ProspectProfile {
            id: "prospect-001",
            name: "Acme Corp",
            persona: Persona::HighIntent,
            industry: "SaaS",
            employee_count: 120,
            funding_stage: "Series B",
        },
        ProspectProfile {
            id: "prospect-002",
            name: "Globex Industries",
            persona: Persona::DeepTechnical,
            industry: "FinTech",
            employee_count: 45,
            funding_stage: "Series A",
        },
        ProspectProfile {
            id: "prospect-003",
            name: "Initech Solutions",
            persona: Persona::TireKicker,
            industry: "Consulting",
            employee_count: 500,
            funding_stage: "Private",
        },
        ProspectProfile {
            id: "prospect-004",
            name: "Stark Digital",
            persona: Persona::EnterpriseEvaluator,
            industry: "Manufacturing",
            employee_count: 2500,
            funding_stage: "Public",
        },
        ProspectProfile {
            id: "prospect-005",
            name: "NovaTech Labs",
            persona: Persona::QuickWin,
            industry: "AI/ML",
            employee_count: 18,
            funding_stage: "Seed",
        },
        ProspectProfile {
            id: "prospect-006",
            name: "Zenith Partners",
            persona: Persona::HighIntent,
            industry: "Legal Tech",
            employee_count: 75,
            funding_stage: "Series A",
        },
        ProspectProfile {
            id: "prospect-007",
            name: "Meridian Health",
            persona: Persona::Dormant,
            industry: "HealthTech",
            employee_count: 300,
            funding_stage: "Series C",
        },
        ProspectProfile {
            id: "prospect-008",
            name: "Cobalt Security",
            persona: Persona::DeepTechnical,
            industry: "Cybersecurity",
            employee_count: 60,
            funding_stage: "Series A",
        },
    ]
}

const EVENT_TYPES: &[&str] = &[
    "pageview",
    "feature_click",
    "docs_read",
    "pricing_visit",
    "demo_request",
    "api_docs",
    "blog_read",
    "comparison_page",
    "case_study",
    "signup_start",
    "contact_form",
    "video_watch",
];

const PAGE_SECTIONS: &[&str] = &[
    "product",
    "pricing",
    "docs",
    "blog",
    "api",
    "enterprise",
    "comparison",
    "case-studies",
    "demo",
    "about",
    "security",
    "changelog",
];

fn persona_event_weights(persona: Persona) -> &'static [(usize, u32)] {
    match persona {
        // (event_type index, relative weight)
        // Heavy pricing, comparison, demo, case study
        Persona::HighIntent => &[
            (0, 15),
            (1, 10),
            (2, 5),
            (3, 25),
            (4, 8),
            (5, 3),
            (6, 5),
            (7, 20),
            (8, 15),
            (9, 5),
            (10, 8),
            (11, 3),
        ],
        // Heavy api_docs, docs_read, feature_click
        Persona::DeepTechnical => &[
            (0, 8),
            (1, 20),
            (2, 25),
            (3, 5),
            (4, 2),
            (5, 30),
            (6, 3),
            (7, 3),
            (8, 2),
            (9, 1),
            (10, 1),
            (11, 2),
        ],
        // Mostly blog, light everything else
        Persona::TireKicker => &[
            (0, 25),
            (1, 3),
            (2, 2),
            (3, 2),
            (4, 0),
            (5, 1),
            (6, 35),
            (7, 2),
            (8, 5),
            (9, 0),
            (10, 0),
            (11, 10),
        ],
        // Broad, deep: enterprise, security, case study, pricing
        Persona::EnterpriseEvaluator => &[
            (0, 12),
            (1, 8),
            (2, 10),
            (3, 15),
            (4, 5),
            (5, 5),
            (6, 5),
            (7, 12),
            (8, 15),
            (9, 3),
            (10, 5),
            (11, 3),
        ],
        // Quick funnel: pageview → pricing → signup
        Persona::QuickWin => &[
            (0, 20),
            (1, 10),
            (2, 5),
            (3, 20),
            (4, 15),
            (5, 2),
            (6, 3),
            (7, 5),
            (8, 3),
            (9, 15),
            (10, 10),
            (11, 2),
        ],
        // Sparse, mostly old pageviews
        Persona::Dormant => &[
            (0, 40),
            (1, 5),
            (2, 5),
            (3, 3),
            (4, 0),
            (5, 2),
            (6, 15),
            (7, 2),
            (8, 5),
            (9, 0),
            (10, 0),
            (11, 5),
        ],
    }
}

fn persona_row_range(persona: Persona) -> (usize, usize) {
    match persona {
        Persona::HighIntent => (8_000, 15_000),
        Persona::DeepTechnical => (10_000, 18_000),
        Persona::TireKicker => (3_000, 6_000),
        Persona::EnterpriseEvaluator => (12_000, 20_000),
        Persona::QuickWin => (2_000, 4_000),
        Persona::Dormant => (500, 1_500),
    }
}

fn persona_section_weights(persona: Persona) -> &'static [(usize, u32)] {
    match persona {
        Persona::HighIntent => &[
            (0, 10),
            (1, 25),
            (2, 5),
            (3, 5),
            (4, 3),
            (5, 8),
            (6, 20),
            (7, 15),
            (8, 8),
            (9, 2),
            (10, 2),
            (11, 2),
        ],
        Persona::DeepTechnical => &[
            (0, 8),
            (1, 3),
            (2, 25),
            (3, 3),
            (4, 30),
            (5, 2),
            (6, 3),
            (7, 2),
            (8, 2),
            (9, 2),
            (10, 10),
            (11, 8),
        ],
        Persona::TireKicker => &[
            (0, 15),
            (1, 2),
            (2, 2),
            (3, 35),
            (4, 1),
            (5, 2),
            (6, 5),
            (7, 5),
            (8, 2),
            (9, 10),
            (10, 1),
            (11, 5),
        ],
        Persona::EnterpriseEvaluator => &[
            (0, 10),
            (1, 12),
            (2, 8),
            (3, 5),
            (4, 5),
            (5, 18),
            (6, 10),
            (7, 15),
            (8, 5),
            (9, 3),
            (10, 12),
            (11, 3),
        ],
        Persona::QuickWin => &[
            (0, 20),
            (1, 20),
            (2, 5),
            (3, 3),
            (4, 2),
            (5, 3),
            (6, 5),
            (7, 3),
            (8, 15),
            (9, 5),
            (10, 2),
            (11, 2),
        ],
        Persona::Dormant => &[
            (0, 20),
            (1, 3),
            (2, 5),
            (3, 20),
            (4, 2),
            (5, 2),
            (6, 5),
            (7, 5),
            (8, 2),
            (9, 15),
            (10, 2),
            (11, 5),
        ],
    }
}

fn weighted_pick(weights: &[(usize, u32)], rng: &mut SmallRng) -> usize {
    let total: u32 = weights.iter().map(|(_, w)| w).sum();
    if total == 0 {
        return weights[0].0;
    }
    let mut roll = rng.random_range(0..total);
    for &(idx, w) in weights {
        if roll < w {
            return idx;
        }
        roll -= w;
    }
    weights.last().unwrap().0
}

fn generate_behavior_events(
    prospects: &[ProspectProfile],
    rng: &mut SmallRng,
) -> Result<DataFrame> {
    let mut prospect_ids = Vec::new();
    let mut timestamps = Vec::new();
    let mut event_types = Vec::new();
    let mut page_sections = Vec::new();
    let mut durations = Vec::new();

    let base_date = NaiveDate::from_ymd_opt(2026, 3, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();

    let window_days = 45;

    for prospect in prospects {
        let (min_rows, max_rows) = persona_row_range(prospect.persona);
        let n_events = rng.random_range(min_rows..=max_rows);
        let event_weights = persona_event_weights(prospect.persona);
        let section_weights = persona_section_weights(prospect.persona);

        let window_seconds = window_days * 86400i64;

        for _ in 0..n_events {
            let offset = rng.random_range(0..window_seconds);
            let ts = base_date + offset;

            // Add session clustering: 60% of events happen in bursts
            let ts = if rng.random_range(0..100) < 60 {
                // Snap to a session: round to nearest 30-min block, then add small jitter
                let session_block = (ts / 1800) * 1800;
                session_block + rng.random_range(0..600)
            } else {
                ts
            };

            // Working hours bias (70% during 8-18, 20% evening, 10% night)
            let hour_roll = rng.random_range(0..100);
            let hour_offset = if hour_roll < 70 {
                rng.random_range(8 * 3600..18 * 3600)
            } else if hour_roll < 90 {
                rng.random_range(18 * 3600..23 * 3600)
            } else {
                rng.random_range(0..8 * 3600)
            };
            let day_start = (ts / 86400) * 86400;
            let ts = day_start + hour_offset;

            let event_idx = weighted_pick(event_weights, rng);
            let section_idx = weighted_pick(section_weights, rng);

            let duration = match EVENT_TYPES[event_idx] {
                "docs_read" | "api_docs" => rng.random_range(30..600),
                "video_watch" => rng.random_range(15..300),
                "pricing_visit" | "comparison_page" => rng.random_range(20..180),
                "demo_request" | "signup_start" | "contact_form" => rng.random_range(60..300),
                "blog_read" | "case_study" => rng.random_range(30..480),
                _ => rng.random_range(5..120),
            };

            prospect_ids.push(prospect.id);
            timestamps.push(ts);
            event_types.push(EVENT_TYPES[event_idx]);
            page_sections.push(PAGE_SECTIONS[section_idx]);
            durations.push(duration as i64);
        }
    }

    let df = DataFrame::new(vec![
        Column::new("prospect_id".into(), &prospect_ids),
        Column::new("timestamp".into(), &timestamps),
        Column::new("event_type".into(), &event_types),
        Column::new("page_section".into(), &page_sections),
        Column::new("duration_s".into(), &durations),
    ])?;

    // Sort by prospect_id, timestamp for temporal feature extraction
    let df = df
        .lazy()
        .sort(["prospect_id", "timestamp"], Default::default())
        .collect()?;

    Ok(df)
}

fn generate_account_context(
    prospects: &[ProspectProfile],
    rng: &mut SmallRng,
) -> Result<DataFrame> {
    let mut ids = Vec::new();
    let mut names = Vec::new();
    let mut industries = Vec::new();
    let mut employee_counts = Vec::new();
    let mut funding_stages = Vec::new();
    let mut tech_stacks = Vec::new();
    let mut email_opens = Vec::new();
    let mut meeting_requests = Vec::new();
    let mut support_tickets = Vec::new();
    let mut linkedin_connections = Vec::new();
    let mut annual_revenue_est = Vec::new();

    let stacks = [
        "AWS, Python, React",
        "GCP, Go, Vue",
        "Azure, .NET, Angular",
        "AWS, Rust, Svelte",
        "Self-hosted, Java, React",
        "Hybrid, TypeScript, Next.js",
        "AWS, Python, FastAPI",
        "GCP, Kotlin, Flutter",
    ];

    for (i, prospect) in prospects.iter().enumerate() {
        ids.push(prospect.id);
        names.push(prospect.name);
        industries.push(prospect.industry);
        employee_counts.push(prospect.employee_count as i64);
        funding_stages.push(prospect.funding_stage);
        tech_stacks.push(stacks[i % stacks.len()]);

        let (opens, meetings, tickets) = match prospect.persona {
            Persona::HighIntent => (
                rng.random_range(15..40),
                rng.random_range(2..5),
                rng.random_range(0..2),
            ),
            Persona::DeepTechnical => (
                rng.random_range(5..15),
                rng.random_range(0..2),
                rng.random_range(3..8),
            ),
            Persona::TireKicker => (rng.random_range(1..5), 0i64, 0),
            Persona::EnterpriseEvaluator => (
                rng.random_range(20..50),
                rng.random_range(3..8),
                rng.random_range(1..4),
            ),
            Persona::QuickWin => (rng.random_range(8..20), rng.random_range(1..3), 0),
            Persona::Dormant => (rng.random_range(0..3), 0i64, 0),
        };

        email_opens.push(opens);
        meeting_requests.push(meetings);
        support_tickets.push(tickets);
        linkedin_connections.push(rng.random_range(1..15) as i64);
        annual_revenue_est.push(prospect.employee_count as i64 * rng.random_range(80_000..150_000));
    }

    let df = DataFrame::new(vec![
        Column::new("prospect_id".into(), &ids),
        Column::new("company_name".into(), &names),
        Column::new("industry".into(), &industries),
        Column::new("employee_count".into(), &employee_counts),
        Column::new("funding_stage".into(), &funding_stages),
        Column::new("tech_stack".into(), &tech_stacks),
        Column::new("email_opens_30d".into(), &email_opens),
        Column::new("meeting_requests_30d".into(), &meeting_requests),
        Column::new("support_tickets_30d".into(), &support_tickets),
        Column::new("linkedin_connections".into(), &linkedin_connections),
        Column::new("annual_revenue_est".into(), &annual_revenue_est),
    ])?;

    Ok(df)
}

fn generate_calendar_availability(rng: &mut SmallRng) -> Result<DataFrame> {
    let actors = ["rep-alice", "rep-bob", "rep-carol"];
    let base_date = NaiveDate::from_ymd_opt(2026, 4, 14).unwrap();

    let mut actor_ids = Vec::new();
    let mut dates = Vec::new();
    let mut slot_starts = Vec::new();
    let mut slot_ends = Vec::new();
    let mut availables = Vec::new();

    // Generate 5 working days of 30-min slots from 08:00-18:00
    for day_offset in 0..5i64 {
        let day = base_date + TimeDelta::days(day_offset);
        let day_str = day.format("%Y-%m-%d").to_string();

        for actor in &actors {
            for slot in 0..20u32 {
                let start_hour = 8 + slot / 2;
                let start_min = if slot % 2 == 0 { 0 } else { 30 };
                let end_min = if start_min == 0 { 30 } else { 0 };
                let end_hour = if end_min == 0 {
                    start_hour + 1
                } else {
                    start_hour
                };

                // 70% available, with some blocked clusters (meetings)
                let available = rng.random_range(0..100) >= 30;

                actor_ids.push(*actor);
                dates.push(day_str.clone());
                slot_starts.push(format!("{start_hour:02}:{start_min:02}"));
                slot_ends.push(format!("{end_hour:02}:{end_min:02}"));
                availables.push(available);
            }
        }
    }

    let df = DataFrame::new(vec![
        Column::new("actor_id".into(), &actor_ids),
        Column::new("date".into(), &dates),
        Column::new("slot_start".into(), &slot_starts),
        Column::new("slot_end".into(), &slot_ends),
        Column::new("available".into(), &availables),
    ])?;

    Ok(df)
}

fn write_parquet(df: &DataFrame, path: &Path) -> Result<()> {
    let mut file = File::create(path)?;
    let mut owned = df.clone();
    ParquetWriter::new(&mut file).finish(&mut owned)?;
    Ok(())
}
