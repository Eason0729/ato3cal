use std::error::Error;
use std::io;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*,
 widgets::*};
use serde::{Deserialize, Serialize};

// --- Model Definitions ---
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LinearModel {
    pub slope: f64,
    pub intercept: f64,
}

impl LinearModel {
    pub fn predict(&self, x: f64) -> f64 {
        self.slope * x + self.intercept
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PredictionSystem {
    pub stopover_same: LinearModel,
    pub direct_same: LinearModel,
    pub stopover_twice: LinearModel,
    pub direct_twice: LinearModel,
    pub stopover_thrice: LinearModel,
    pub direct_thrice: LinearModel,
}

// --- App State ---

enum InputMode {
    Normal,
    Editing,
}

#[derive(PartialEq, Debug)]
enum FocusedField {
    MyCityPoint,
    PlaneSeating,
    Scenario,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Scenario {
    StopoverSame,
    DirectSame,
    StopoverTwice,
    DirectTwice,
    StopoverThrice,
    DirectThrice,
}

impl Scenario {
    fn to_str(&self) -> &str {
        match self {
            Scenario::StopoverSame => "Stopover (Both Cities Same Size)",
            Scenario::DirectSame => "Direct (Both Cities Same Size)",
            Scenario::StopoverTwice => "Stopover (One City Twice as Big)",
            Scenario::DirectTwice => "Direct (One City Twice as Big)",
            Scenario::StopoverThrice => "Stopover (One City 3x Bigger)",
            Scenario::DirectThrice => "Direct (One City 3x Bigger)",
        }
    }
    
    fn all() -> Vec<Scenario> {
        vec![
            Scenario::StopoverSame,
            Scenario::DirectSame,
            Scenario::StopoverTwice,
            Scenario::DirectTwice,
            Scenario::StopoverThrice,
            Scenario::DirectThrice,
        ]
    }
}

struct App {
    my_city_point: String,
    plane_seating: String,
    selected_scenario_idx: usize,
    input_mode: InputMode,
    focused_field: FocusedField,
    prediction_system: PredictionSystem,
    scenarios: Vec<Scenario>,
}

impl App {
    fn new(sys: PredictionSystem) -> App {
        App {
            my_city_point: String::new(),
            plane_seating: String::new(),
            selected_scenario_idx: 0,
            input_mode: InputMode::Normal,
            focused_field: FocusedField::MyCityPoint,
            prediction_system: sys,
            scenarios: Scenario::all(),
        }
    }
    
    fn get_current_model(&self) -> &LinearModel {
        let sc = self.scenarios[self.selected_scenario_idx];
        match sc {
            Scenario::StopoverSame => &self.prediction_system.stopover_same,
            Scenario::DirectSame => &self.prediction_system.direct_same,
            Scenario::StopoverTwice => &self.prediction_system.stopover_twice,
            Scenario::DirectTwice => &self.prediction_system.direct_twice,
            Scenario::StopoverThrice => &self.prediction_system.stopover_thrice,
            Scenario::DirectThrice => &self.prediction_system.direct_thrice,
        }
    }
    
    fn calculate(&self) -> Option<(f64, f64)> {
        let seating: f64 = self.plane_seating.parse().ok()?;
        let my_point: f64 = self.my_city_point.parse().ok()?;
        
        let model = self.get_current_model();
        let required_sum = model.predict(seating);
        let other_city_needed = required_sum - my_point;
        
        Some((required_sum, other_city_needed))
    }

    fn next_field(&mut self) {
        self.focused_field = match self.focused_field {
            FocusedField::MyCityPoint => FocusedField::PlaneSeating,
            FocusedField::PlaneSeating => FocusedField::Scenario,
            FocusedField::Scenario => FocusedField::MyCityPoint,
        };
    }

    fn prev_field(&mut self) {
        self.focused_field = match self.focused_field {
            FocusedField::MyCityPoint => FocusedField::Scenario,
            FocusedField::PlaneSeating => FocusedField::MyCityPoint,
            FocusedField::Scenario => FocusedField::PlaneSeating,
        };
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // Load Model
    let model_data = include_bytes!("../model.bin");
    let sys: PredictionSystem = bincode::deserialize(model_data)?;

    // Setup Terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run App
    let app = App::new(sys);
    let res = run_app(&mut terminal, app);

    // Restore Terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press { continue; }
            
            match app.input_mode {
                InputMode::Normal => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Tab | KeyCode::Down => app.next_field(),
                    KeyCode::BackTab | KeyCode::Up => app.prev_field(),
                    KeyCode::Enter => {
                        if app.focused_field == FocusedField::Scenario {
                            // Cycle scenario
                             if app.selected_scenario_idx + 1 >= app.scenarios.len() {
                                app.selected_scenario_idx = 0;
                            } else {
                                app.selected_scenario_idx += 1;
                            }
                        } else {
                            app.input_mode = InputMode::Editing;
                        }
                    },
                    KeyCode::Left => {
                         if app.focused_field == FocusedField::Scenario {
                            if app.selected_scenario_idx > 0 {
                                app.selected_scenario_idx -= 1;
                            } else {
                                app.selected_scenario_idx = app.scenarios.len() - 1;
                            }
                         }
                    },
                    KeyCode::Right => {
                         if app.focused_field == FocusedField::Scenario {
                             if app.selected_scenario_idx + 1 >= app.scenarios.len() {
                                app.selected_scenario_idx = 0;
                            } else {
                                app.selected_scenario_idx += 1;
                            }
                         }
                    },
                    _ => {} // Ignore other keys
                },
                InputMode::Editing => match key.code {
                    KeyCode::Enter | KeyCode::Esc => app.input_mode = InputMode::Normal,
                    KeyCode::Char(c) => {
                        match app.focused_field {
                            FocusedField::MyCityPoint => {
                                if c.is_ascii_digit() || c == '.' {
                                    app.my_city_point.push(c);
                                }
                            },
                            FocusedField::PlaneSeating => {
                                if c.is_ascii_digit() || c == '.' {
                                    app.plane_seating.push(c);
                                }
                            },
                            _ => {} // Should not happen
                        }
                    },
                    KeyCode::Backspace => {
                         match app.focused_field {
                            FocusedField::MyCityPoint => { app.my_city_point.pop(); },
                            FocusedField::PlaneSeating => { app.plane_seating.pop(); },
                            _ => {} // Should not happen
                        }
                    }
                    _ => {} // Ignore other keys
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(1), // Title
                Constraint::Length(3), // My City Point
                Constraint::Length(3), // Plane Seating
                Constraint::Length(3), // Scenario
                Constraint::Min(5),    // Result
                Constraint::Length(1), // Footer
            ]
            .as_ref(),
        )
        .split(f.size());

    let title = Paragraph::new("Air Tycoon Online 3 Calculator (Model based on data.csv)")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    f.render_widget(title, chunks[0]);

    // Helper to style active fields
    let get_style = |field: FocusedField| {
        if app.focused_field == field {
            match app.input_mode {
                InputMode::Editing => Style::default().fg(Color::Yellow),
                InputMode::Normal => Style::default().fg(Color::Green),
            }
        } else {
            Style::default()
        }
    };

    // My City Point Input
    let my_city_txt = Paragraph::new(app.my_city_point.as_str())
        .style(get_style(FocusedField::MyCityPoint))
        .block(Block::default().borders(Borders::ALL).title("My City Points"));
    f.render_widget(my_city_txt, chunks[1]);

    // Plane Seating Input
    let plane_txt = Paragraph::new(app.plane_seating.as_str())
        .style(get_style(FocusedField::PlaneSeating))
        .block(Block::default().borders(Borders::ALL).title("Plane Max Seating"));
    f.render_widget(plane_txt, chunks[2]);

    // Scenario Selector
    let scenario_str = app.scenarios[app.selected_scenario_idx].to_str();
    let scenario_widget = Paragraph::new(format!(" < {} > ", scenario_str))
        .style(get_style(FocusedField::Scenario))
        .block(Block::default().borders(Borders::ALL).title("Scenario (Left/Right to Change)"));
    f.render_widget(scenario_widget, chunks[3]);

    // Result Area
    let result_text = if let Some((req_sum, needed)) = app.calculate() {
        format!(
            "Required Total Sum: {:.2}\n\n>>> Other City Needed: {:.2} <<<",
            req_sum,
            needed
        )
    } else {
        String::from("Please enter valid numbers.")
    };
    
    let result_widget = Paragraph::new(result_text)
        .style(Style::default().fg(Color::White))
        .block(Block::default().borders(Borders::ALL).title("Prediction"));
    f.render_widget(result_widget, chunks[4]);

    let footer = Paragraph::new("Press 'q' to quit. 'Enter' to edit. Up/Down/Tab to navigate.")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(footer, chunks[5]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_calculation() {
        let dummy_model = LinearModel { slope: 2.0, intercept: 100.0 };
        let sys = PredictionSystem {
            stopover_same: dummy_model.clone(),
            direct_same: dummy_model.clone(),
            stopover_twice: dummy_model.clone(),
            direct_twice: dummy_model.clone(),
            stopover_thrice: dummy_model.clone(),
            direct_thrice: dummy_model.clone(),
        };
        
        let mut app = App::new(sys);
        app.my_city_point = String::from("500");
        app.plane_seating = String::from("100");
        
        // Prediction: 2.0 * 100 + 100 = 300.
        // Other City Needed: 300 - 500 = -200.
        
        let res = app.calculate();
        assert!(res.is_some());
        let (req, needed) = res.unwrap();
        assert!((req - 300.0).abs() < 1e-6);
        assert!((needed - -200.0).abs() < 1e-6);
    }
}