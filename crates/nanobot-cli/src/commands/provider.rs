//! Provider command

use nanobot_providers::PROVIDERS;

/// List all available providers
pub fn list() {
    println!("Available LLM Providers:");
    println!();

    // Group by type
    let (gateways, standard, local, auxiliary): (Vec<_>, Vec<_>, Vec<_>, Vec<_>) =
        PROVIDERS.iter().cloned().partition4(|spec| {
            if spec.is_gateway {
                0
            } else if spec.is_local {
                2
            } else if !spec.keywords.is_empty() && spec.keywords[0] == "groq" {
                3
            } else {
                1
            }
        });

    println!("=== Gateways ===");
    for spec in gateways {
        println!("  {} - {}", spec.name, spec.label());
        if let Some(base) = spec.default_api_base {
            println!("    Base: {}", base);
        }
    }

    println!();
    println!("=== Standard ===");
    for spec in standard {
        println!("  {} - {}", spec.name, spec.label());
    }

    println!();
    println!("=== Local ===");
    for spec in local {
        println!("  {} - {}", spec.name, spec.label());
        if let Some(base) = spec.default_api_base {
            println!("    Default: {}", base);
        }
    }

    println!();
    println!("=== Auxiliary ===");
    for spec in auxiliary {
        println!("  {} - {}", spec.name, spec.label());
    }

    println!();
    println!("Configure providers in ~/.nanobot/config.json under 'providers' section.");
}

// Helper for partition4
trait IteratorExt: Iterator {
    fn partition4<F>(self, f: F) -> (Vec<Self::Item>, Vec<Self::Item>, Vec<Self::Item>, Vec<Self::Item>)
    where
        F: Fn(&Self::Item) -> usize,
        Self: Sized,
    {
        let mut groups = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for item in self {
            match f(&item) {
                0 => groups.0.push(item),
                1 => groups.1.push(item),
                2 => groups.2.push(item),
                _ => groups.3.push(item),
            }
        }
        groups
    }
}

impl<I: Iterator> IteratorExt for I {}
