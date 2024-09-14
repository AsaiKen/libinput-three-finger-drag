extern crate regex;

use regex::Regex;
use std::io::{self, BufRead};
use std::iter::Iterator;
use std::process::{Command, Stdio};

mod xdo_handler;

fn main() {
    let output = Command::new("stdbuf")
        .arg("-o0")
        .arg("libinput")
        .arg("debug-events")
        .stdout(Stdio::piped())
        .spawn()
        .expect("can not exec libinput")
        .stdout
        .expect("libinput has no stdout");

    let mut xdo_handler = xdo_handler::start_handler();

    let swipe_acceleration = 3.0;
    let mut xsum: f32 = 0.0;
    let mut ysum: f32 = 0.0;
    let pattern = Regex::new(r"[\s]+|/|\(").unwrap();

    let mut hscrolling = false;

    for line in io::BufReader::new(output).lines() {
        let line = line.unwrap();
        let parts: Vec<&str> = pattern.split(&line).filter(|c| !c.is_empty()).collect();
        let action = parts[1];

        if line.contains("GESTURE_") {
            hscrolling = false;

            // event10  GESTURE_SWIPE_UPDATE +3.769s	4  0.25/ 0.48 ( 0.95/ 1.85 unaccelerated)
            let finger = parts[3];
            if finger != "3" && !action.starts_with("GESTURE_HOLD"){
                xdo_handler.mouse_up(1);
                continue;
            }
            let cancelled = parts.len() > 4 && parts[4] == "cancelled";

            match action {
                "GESTURE_SWIPE_BEGIN" => {
                    xsum = 0.0;
                    ysum = 0.0;
                    xdo_handler.mouse_down(1);
                    println!("{}", "Swipe");
                }
                "GESTURE_SWIPE_UPDATE" => {
                    let x: f32 = parts[4].parse().unwrap();
                    let y: f32 = parts[5].parse().unwrap();
                    xsum += x * swipe_acceleration;
                    ysum += y * swipe_acceleration;
                    if xsum.abs() > 1.0 || ysum.abs() > 1.0 {
                        xdo_handler.move_mouse_relative(xsum as i32, ysum as i32);
                        xsum = 0.0;
                        ysum = 0.0;
                    }
                }
                "GESTURE_SWIPE_END" => {
                    xdo_handler.move_mouse_relative(xsum as i32, ysum as i32);
                    if cancelled {
                        xdo_handler.mouse_up(1);
                    } else {
                        xdo_handler.mouse_up_delay(1, 600);
                    }
                }
                "GESTURE_HOLD_BEGIN" => {
                    // Ignore
                }
                "GESTURE_HOLD_END" => {
                    // Ignore accidental holds when repositioning
                    if !cancelled {
                        xdo_handler.mouse_up(1);
                    }
                }
                _ => {
                    // GESTURE_PINCH_*,
                    xdo_handler.mouse_up(1);
                }
            }
        } else if line.contains("POINTER_SCROLL_FINGER") {
            // 2本指の左右スワイプを処理
            // event9   POINTER_SCROLL_FINGER   +0.247s	vert 0.00/0.0 horiz -1.97/0.0* (finger)
            if parts.len() >= 8 {
                let h_scroll: f32 = parts[7].parse().unwrap_or(0.0);
                if hscrolling {
                    // hscrollは1回だけ
                } else if h_scroll >= 15.0 {
                    if is_chrome_focused() {
                        // 右スワイプ（Alt+Left）
                        println!("{}", "Alt+Left");
                        xdo_handler.key_combo("Alt+Left");
                    }
                    hscrolling = true;
                } else if h_scroll <= -15.0 {
                    // 左スワイプ（Alt+Right）
                    if is_chrome_focused() {
                        println!("{}", "Alt+Right");
                        xdo_handler.key_combo("Alt+Right");
                    }
                    hscrolling = true;
                }
            }
        } else {
            xdo_handler.mouse_up(1);
            hscrolling = false;
        }
    }
}

fn is_chrome_focused() -> bool {
    // xpropコマンドでアクティブなウィンドウIDを取得
    let output = Command::new("xprop")
        .arg("-root")
        .arg("_NET_ACTIVE_WINDOW")
        .output()
        .expect("Failed to execute xprop");

    if !output.status.success() {
        return false;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let window_id_line = stdout.trim();

    // ウィンドウIDを正規表現で抽出
    let re = Regex::new(r"_NET_ACTIVE_WINDOW\(WINDOW\): window id # (0x[0-9a-fA-F]+)").unwrap();
    let caps = re.captures(window_id_line);

    let window_id = if let Some(caps) = caps {
        caps.get(1).unwrap().as_str()
    } else {
        return false;
    };

    // 取得したウィンドウIDでWM_CLASSを調べる
    let output = Command::new("xprop")
        .arg("-id")
        .arg(window_id)
        .output()
        .expect("Failed to execute xprop");

    if !output.status.success() {
        return false;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.starts_with("WM_CLASS(") {
            if line.contains("google-chrome") || line.contains("Google-chrome") {
                return true;
            }
        }
    }

    false
}
