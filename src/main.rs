// src/main.rs
use crossterm::{
    event::{read, Event, KeyCode},
    style::{Color, ResetColor, SetForegroundColor},
    terminal::{self},
    ExecutableCommand,
};
use std::error::Error;
use std::fs;
use std::io::{self, stdout, Write};
use std::time::Instant;

fn parse_test_case(block: &str) -> Result<(Vec<String>, Vec<Vec<f64>>), Box<dyn Error>> {
    let mut lines = block
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::trim);
    let header = lines
        .next()
        .ok_or("Empty input: first line must list city names")?;
    let cities: Vec<String> = header
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let n = cities.len();
    if n == 0 {
        return Err("No cities found in header".into());
    }

    let mut dist_mat = vec![vec![0.; n]; n];
    for i in 0..n {
        let row = lines
            .next()
            .ok_or(format!("Expected {} rows of distances, got fewer", n))?;
        let parts: Vec<&str> = row.split_whitespace().collect();
        if parts.len() != n {
            return Err(format!(
                "Row {} has {} columns, but {} expected",
                i + 1,
                parts.len(),
                n
            )
            .into());
        }
        for j in 0..n {
            let d: f64 = parts[j].parse()?;
            dist_mat[i][j] = d;
        }
    }
    Ok((cities, dist_mat))
}

fn solve_tsp(dist: &[Vec<f64>]) -> (f64, Vec<usize>) {
    let n = dist.len();
    let max_mask = 1 << n;
    let inf = f64::INFINITY;
    let mut dp = vec![vec![inf; n]; max_mask];
    let mut parent = vec![vec![None; n]; max_mask];
    dp[1][0] = 0.0;
    for mask in 1..max_mask {
        if (mask & 1) == 0 {
            continue;
        }
        for u in 0..n {
            if (mask & (1 << u)) == 0 {
                continue;
            }
            if u == 0 && mask != 1 {
                continue;
            }
            let prev_mask = mask ^ (1 << u);
            if prev_mask == 0 && u == 0 {
                continue;
            }
            if u == 0 {
                continue;
            }
            let mut best_cost = inf;
            let mut best_prev = None;
            for v in 0..n {
                if (prev_mask & (1 << v)) == 0 {
                    continue;
                }
                let cost = dp[prev_mask][v] + dist[v][u];
                if cost < best_cost {
                    best_cost = cost;
                    best_prev = Some(v);
                }
            }
            if best_prev.is_some() {
                dp[mask][u] = best_cost;
                parent[mask][u] = best_prev;
            }
        }
    }
    let full_mask = max_mask - 1;
    let mut best_cost = inf;
    let mut last = None;
    for u in 1..n {
        let cost = dp[full_mask][u] + dist[u][0];
        if cost < best_cost {
            best_cost = cost;
            last = Some(u);
        }
    }
    let mut path = Vec::with_capacity(n + 1);
    let mut cur = last.unwrap();
    let mut mask = full_mask;
    while let Some(prev) = parent[mask][cur] {
        path.push(cur);
        mask ^= 1 << cur;
        cur = prev;
    }
    path.push(0);
    path.reverse();
    path.push(0);
    (best_cost, path)
}

#[derive(Clone)]
enum AsciiGraph {
    Empty,
    Path,
    PathSolution { order: f64 },
    TagName { char: char }
}

fn draw_ascii_graph(
    cities: &[String],
    solution_path: &[usize],
    grid_w: usize,
    grid_h: usize,
) -> io::Result<()> {
    // Create a char buffer of size grid_w × grid_h, initialized with spaces
    let mut buf: Vec<Vec<AsciiGraph>> = vec![vec![AsciiGraph::Empty; grid_w]; grid_h];
    let n = cities.len();
    let cx = (grid_w / 2) as f32;
    let cy = (grid_h / 2) as f32;
    let mut pts: Vec<(usize, usize)> = Vec::new();
    for i in 0..n {
        let theta = 2.0 * std::f32::consts::PI * (i as f32) / (n as f32);
        let x = (cx + (grid_w as f32) * 0.45 * theta.cos()).round() as isize;
        let y = (cy + (grid_h as f32) * 0.45 * theta.sin()).round() as isize;
        let (xi, yi) = (x.clamp(0, (grid_w - 1) as isize) as usize, y.clamp(0, (grid_h - 1) as isize) as usize);
        pts.push((xi, yi));
    }
    fn bresenham_line(x0: usize, y0: usize, x1: usize, y1: usize) -> Vec<(usize, usize)> {
        let mut pts = Vec::new();
        let (mut x0, mut y0) = (x0 as isize, y0 as isize);
        let (x1, y1) = (x1 as isize, y1 as isize);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            pts.push((x0 as usize, y0 as usize));
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
        pts
    }
    for i in 0..n {
        for j in i + 1..n {
            for &(x, y) in &bresenham_line(pts[i].0, pts[i].1, pts[j].0, pts[j].1) {
                buf[y][x] = AsciiGraph::Path;
            }
        }
    }
    let mut order = 0.0;
    for w in solution_path.windows(2) {
        let i = w[0];
        let j = w[1];
        let line_pts = bresenham_line(pts[i].0, pts[i].1, pts[j].0, pts[j].1);
        let mut k = 0.0;
        for &(x, y) in &line_pts {
            buf[y][x] = AsciiGraph::PathSolution { order: order + k };
            k += 1.0 / (line_pts.len() as f64);
        }
        order += 1.0;
    }
    for (i, &(x, y)) in pts.iter().enumerate() {
        let city_name = &cities[i];
        let start_x = if x >= city_name.len() / 2 {
            x - city_name.len() / 2
        } else {
            0
        };
        for (j, ch) in city_name.chars().enumerate() {
            if let Some(row) = buf.get_mut(y) {
                if let Some(cell) = row.get_mut(start_x + j) {
                    *cell = AsciiGraph::TagName { char: ch };
                }
            }
        }
    }

    fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;
        let (r1, g1, b1) = match h as u32 {
            0..=59 => (c, x, 0.0),
            60..=119 => (x, c, 0.0),
            120..=179 => (0.0, c, x),
            180..=239 => (0.0, x, c),
            240..=299 => (x, 0.0, c),
            300..=359 => (c, 0.0, x),
            _ => (0.0, 0.0, 0.0),
        };
        (r1 + m, g1 + m, b1 + m)
    }
    let hue_mul = (360.0 / (solution_path.len() as f64)).max(30.0);
    let mut stdout = stdout();
    for row in buf {
        for ch in row {
            match ch {
                AsciiGraph::Empty => {
                    write!(stdout, " ")?
                },
                AsciiGraph::Path { .. } => {
                    stdout.execute(SetForegroundColor(Color::DarkGrey))?;
                    write!(stdout, ".")?;
                    stdout.execute(ResetColor)?;
                },
                AsciiGraph::PathSolution { order, .. } => {
                    let hue = (order * hue_mul) % 360.0;
                    let (r, g, b) = hsv_to_rgb(hue, 1.0, 1.0);
                    let color = Color::Rgb {
                        r: (r * 255.0) as u8,
                        g: (g * 255.0) as u8,
                        b: (b * 255.0) as u8,
                    };
                    stdout.execute(SetForegroundColor(color))?;
                    write!(stdout, "#")?;
                    stdout.execute(ResetColor)?;
                },
                AsciiGraph::TagName { char } => {
                    stdout.execute(SetForegroundColor(Color::White))?;
                    stdout.execute(crossterm::style::SetAttribute(
                        crossterm::style::Attribute::Bold,
                    ))?;
                    write!(stdout, "{char}")?;
                    stdout.execute(ResetColor)?;
                }
            }
        }
        writeln!(stdout)?;
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    terminal::enable_raw_mode()?;

    loop {
        println!("Traveling Salesman Problem Solver");
        println!("Pilih masukan test case:");
        println!("  1) Melalui txt file");
        println!("  2) Paste test case secara langsung");
        println!("Pilih '1' atau '2' (atau 'q' untuk keluar):");

        let choice = loop {
            if let Event::Key(key_event) = read()? {
                match key_event.code {
                    KeyCode::Char('1') => break '1',
                    KeyCode::Char('2') => break '2',
                    KeyCode::Char('q') | KeyCode::Esc => {
                        terminal::disable_raw_mode()?;
                        return Ok(());
                    }
                    _ => {}
                }
            }
        };

        let block = match choice {
            '1' => {
                println!("Masukkan input file path: ");
                terminal::disable_raw_mode()?;
                let mut path = String::new();
                io::stdin().read_line(&mut path)?;
                let path = path.trim();
                terminal::enable_raw_mode()?;
                match fs::read_to_string(path) {
                    Ok(s) => s,
                    Err(e) => {
                        println!("Error saat membaca file: {e}");
                        println!("Tekan tombol apa saja untuk kembali...");
                        let _ = read();
                        continue;
                    }
                }
            }
            '2' => {
                terminal::disable_raw_mode()?;
                let mut pasted = String::new();
                println!("Paste test case secara langsung (akhiri dengan baris kosong):");
                loop {
                    let mut line = String::new();
                    io::stdin().read_line(&mut line)?;
                    if line.trim().is_empty() {
                        break;
                    }
                    pasted.push_str(&line);
                }
                terminal::enable_raw_mode()?;
                pasted
            }
            _ => unreachable!(),
        };

        let (cities, dist_mat) = match parse_test_case(&block) {
            Ok(x) => x,
            Err(e) => {
                println!("Error saat parsing: {e}");
                println!("Tekan tombol apa saja untuk kembali...");
                let _ = read();
                continue;
            }
        };

        println!("Menyelesaikan TSP untuk {} kota...", cities.len());
        let t0 = Instant::now();
        let (best_cost, best_path) = solve_tsp(&dist_mat);
        let elapsed = t0.elapsed();

        println!("");
        println!("Cost Optimal Tour: {:.3}", best_cost);
        println!("Urutan Tour:");
        for &idx in &best_path {
            print!("{} → ", cities[idx]);
        }
        println!("(end)");
        println!("\nWaktu: {:.3}ms", elapsed.as_secs_f64() * 1000.0);

        draw_ascii_graph(&cities, &best_path, 80, 24)?;
        println!("");
        println!("Tekan tombol apa saja untuk melanjutkan...");
        let _ = read();

        loop {
            println!("Simpan solusi ke file? (y/n)");
            if let Event::Key(key_event) = read()? {
                match key_event.code {
                    KeyCode::Char('y') => {
                        terminal::disable_raw_mode()?;
                        println!("Masukkan output file path: ");
                        let mut out_path = String::new();
                        io::stdin().read_line(&mut out_path)?;
                        let out_path = out_path.trim();
                        let mut out_contents = String::new();
                        out_contents.push_str(&format!("Cost Optimal Tour: {:.3}\n", best_cost));
                        out_contents.push_str("Urutan Tour:\n");
                        for &idx in &best_path {
                            out_contents.push_str(&cities[idx]);
                            out_contents.push_str(" ");
                        }
                        out_contents.push('\n');
                        match fs::write(out_path, out_contents) {
                            Ok(_) => {
                                println!("Disimpan ke {out_path}");
                            }
                            Err(e) => {
                                println!("Error saat menyimpan file: {e}");
                            }
                        }
                        println!("Tekan tombol apa saja untuk melanjutkan...");
                        let _ = io::stdin().read_line(&mut String::new());
                        terminal::enable_raw_mode()?;
                        break;
                    }
                    KeyCode::Char('n') => break,
                    _ => {}
                }
            }
        }

        loop {
            println!("Jalankan test case lainnya? (y/n)");
            if let Event::Key(key_event) = read()? {
                match key_event.code {
                    KeyCode::Char('y') => break,
                    KeyCode::Char('n') => {
                        terminal::disable_raw_mode()?;
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
    }
}
