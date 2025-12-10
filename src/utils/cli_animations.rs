use super::{icons, theme};
use colored::*;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

fn format_timestamp(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        let half = max / 2 - 2;
        format!("{}...{}", &s[..half], &s[s.len() - half..])
    } else {
        s.to_string()
    }
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

struct Box {
    color: (u8, u8, u8),
}

impl Box {
    fn new(color: (u8, u8, u8)) -> Self {
        Self { color }
    }

    fn top(&self) {
        println!("    {}", "┌─────────────────────────────────────────────────────┐"
            .truecolor(self.color.0, self.color.1, self.color.2));
    }

    fn mid(&self) {
        println!("    {}", "├─────────────────────────────────────────────────────┤"
            .truecolor(self.color.0, self.color.1, self.color.2));
    }

    fn bottom(&self) {
        println!("    {}", "└─────────────────────────────────────────────────────┘"
            .truecolor(self.color.0, self.color.1, self.color.2));
    }

    fn row(&self, content: &str) {
        println!("    {} {} {}", 
            "|".truecolor(self.color.0, self.color.1, self.color.2),
            content,
            "|".truecolor(self.color.0, self.color.1, self.color.2));
    }

    fn title(&self, icon: &str, text: &str) {
        println!("    {} {} {} {}",
            "|".truecolor(self.color.0, self.color.1, self.color.2),
            icon,
            text.truecolor(self.color.0, self.color.1, self.color.2).bold(),
            "|".truecolor(self.color.0, self.color.1, self.color.2));
    }
}

pub struct Cli;

impl Cli {
    pub fn banner() {
        println!();
        let lines = [
        "           █████████████████████████████████████████████████",
        "         ████████████████████████████████████████████████",
        "       ████████████████████████████████████████████████",
        "     ████████████████████████████████████████████████",
        "                                                     ",
        "    █████████████████████████████████████████████████",
        "      █████████████████████████████████████████████████",
        "        █████████████████████████████████████████████████",
        "          █████████████████████████████████████████████████",
        "                                                           ",
        "           ████████████████████████████████████████████████",
        "         ███████████████████████████████████████████████",
        "       ███████████████████████████████████████████████",
        "     ███████████████████████████████████████████████",
        ];

        for (i, line) in lines.iter().enumerate() {
            let c = theme::BANNER_GRADIENT.get(i).unwrap_or(&theme::ACCENT);
            println!("{}", line.truecolor(c.0, c.1, c.2));
        }

        println!();
        println!("  {}", "Solana Indexer".truecolor(theme::ACCENT.0, theme::ACCENT.1, theme::ACCENT.2).bold());
        println!();

        print!("    {}", "Initializing".bright_white());
        for _ in 0..4 {
            print!(".");
            let _ = io::stdout().flush();
            thread::sleep(Duration::from_millis(120));
        }
        println!(" {}", "Ready".bright_green().bold());
        println!();
    }

    pub fn success(msg: &str) {
        println!("    {} {}", icons::SUCCESS.bright_green(), msg.bright_green());
    }

    pub fn warning(msg: &str) {
        println!("    {} {}", icons::WARNING.bright_yellow(), msg.bright_yellow());
    }

    pub fn info(msg: &str) {
        println!("    {} {}", icons::INFO.bright_cyan(), msg.bright_cyan());
    }

    pub fn error(context: &str, msg: &str) {
        let b = Box::new(theme::ERROR);
        println!();
        b.top();
        b.title(icons::ERROR, &format!("ERROR: {}", context));
        b.mid();
        b.row(msg);
        b.bottom();
        println!();
    }

    pub fn slot(slot: &crate::core::types::Slot, leader: Option<&str>) {
        let status = match &slot.status {
            crate::core::types::SlotStatus::Confirmed => "Confirmed",
            crate::core::types::SlotStatus::Processed => "Processed",
            crate::core::types::SlotStatus::Finalized => "Finalized",
        };
        let parent = slot.parent.map(|p| p.to_string()).unwrap_or_else(|| "-".into());
        let time = format_timestamp(slot.timestamp);

        let b = Box::new(theme::CYAN);
        println!();
        b.top();
        b.title(icons::SLOT, "SLOT");
        b.mid();
        b.row(&format!("Slot: {}  Parent: {}", 
            slot.slot.to_string().bright_white().bold(), 
            parent.bright_yellow()));
        b.row(&format!("Status: {}  Time: {}", 
            status.bright_green(), 
            time.bright_black()));
        if let Some(l) = leader {
            b.row(&format!("Leader: {}", truncate(l, 20).bright_cyan()));
        }
        b.bottom();
    }

    pub fn transaction(sig: &str, slot: u64, success: bool, fee: u64, program: &str, ix: usize, cu: u64) {
        let (label, color) = if success {
            ("TX CONFIRMED", theme::SUCCESS)
        } else {
            ("TX FAILED", theme::ERROR)
        };

        let b = Box::new(color);
        println!();
        b.top();
        b.title(icons::TRANSACTION, label);
        b.mid();
        b.row(&format!("Sig: {}", truncate(sig, 24).bright_white()));
        b.row(&format!("Slot: {}  Status: {}", 
            slot.to_string().bright_white(),
            if success { "OK".bright_green() } else { "FAIL".bright_red() }));
        b.row(&format!("Program: {}", truncate(program, 24).bright_yellow()));
        b.row(&format!("Instructions: {}  CU: {}  Fee: {}", 
            ix.to_string().bright_cyan(),
            cu.to_string().bright_blue(),
            fee.to_string().bright_yellow()));
        b.bottom();
    }

    pub fn account(acc: &crate::core::types::AccountState) {
        let sol = acc.lamports as f64 / 1_000_000_000.0;
        let exec = if acc.executable { "Yes".bright_green() } else { "No".bright_black() };

        let b = Box::new(theme::CYAN);
        println!();
        b.top();
        b.title(icons::DATABASE, "ACCOUNT");
        b.mid();
        b.row(&format!("Address: {}", truncate(&acc.address, 20).bright_white()));
        b.row(&format!("Balance: {} ({:.4} SOL)", format_number(acc.lamports).bright_yellow(), sol));
        b.row(&format!("Owner: {}", truncate(&acc.owner, 24).bright_white()));
        b.row(&format!("Executable: {}  Data: {} bytes", exec, acc.data.len()));
        b.bottom();
    }

    pub fn account_change(addr: &str, prev: u64, curr: u64, slot: u64) {
        let diff = curr as i64 - prev as i64;
        let (label, color) = if diff > 0 {
            ("BALANCE +", theme::SUCCESS)
        } else if diff < 0 {
            ("BALANCE -", theme::ACCENT)
        } else {
            ("DATA CHANGED", theme::WARNING)
        };

        let b = Box::new(color);
        println!();
        b.top();
        b.title(icons::MONEY, label);
        b.mid();
        b.row(&format!("Address: {}", truncate(addr, 20).bright_white()));
        b.row(&format!("{} → {}", format_number(prev), format_number(curr).bright_yellow()));
        b.row(&format!("Slot: {}", slot.to_string().bright_white()));
        b.bottom();
    }

    pub fn wallet(addr: &str, name: &str) {
        println!("    {} {} ({})", 
            icons::WALLET.bright_cyan(),
            truncate(addr, 16).bright_white(),
            name.bright_cyan());
    }

    pub fn connecting(url: &str) {
        for dots in [".", "..", "...", "...."] {
            print!("\r    {} Connecting{} ", icons::CONNECTION.bright_yellow(), dots);
            let _ = io::stdout().flush();
            thread::sleep(Duration::from_millis(200));
        }
        println!("\r    {} Connected to {}", icons::COMPLETE.bright_green(), url.bright_blue());
    }
}