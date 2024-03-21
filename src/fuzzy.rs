use std::io;

use anyhow::{bail, Result};
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{self, disable_raw_mode},
    ExecutableCommand,
};
use ratatui::{
    prelude::{Backend, Constraint, CrosstermBackend, Direction, Layout},
    style::{Color, Style},
    widgets::{List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use tui_input::{backend::crossterm::EventHandler, Input};

#[derive(Clone)]
pub struct FuzzyFinderItem<T: Clone> {
    pub value: T,
    pub display: String,
}

pub fn run_fuzzy_finder<T: Clone>(list: Vec<FuzzyFinderItem<T>>) -> Result<T> {
    crossterm::terminal::enable_raw_mode()?;

    let mut stdout = io::stdout();

    stdout
        .execute(terminal::EnterAlternateScreen)?
        .execute(event::EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);

    let mut terminal = Terminal::new(backend)?;

    // NOTE: We don't use '?' here because we still need to disable raw mode afterwarsd
    let chosen_or_err = run_app(
        &mut terminal,
        State {
            input_widget: Input::default(),
            list,
            list_state: ListState::default(),
            filtered: vec![],
        },
    );

    disable_raw_mode()?;

    terminal
        .backend_mut()
        .execute(terminal::LeaveAlternateScreen)?
        .execute(event::DisableMouseCapture)?;

    terminal.show_cursor()?;

    chosen_or_err
}

fn run_app<B: Backend, T: Clone>(terminal: &mut Terminal<B>, mut state: State<T>) -> Result<T> {
    loop {
        state.filtered = fuzzy_find_match(state.input_widget.value(), &state.list);

        match state.list_state.selected() {
            Some(selected) => {
                if selected >= state.filtered.len() {
                    state
                        .list_state
                        .select(Some(state.filtered.len().max(1) - 1));
                }
            }

            None => {
                if !state.filtered.is_empty() {
                    state.list_state.select(Some(0));
                }
            }
        }

        terminal.draw(|f| draw_ui(f, &mut state))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Enter => {
                    if let Some(selected) = state.list_state.selected() {
                        return Ok(state.filtered[selected].value.clone());
                    }
                }

                KeyCode::Esc => {
                    bail!("User cancelled");
                }

                KeyCode::Up => match state.list_state.selected() {
                    Some(selected) => {
                        if selected > 0 {
                            state.list_state.select(Some(selected - 1));
                        }
                    }

                    None => {
                        if !state.filtered.is_empty() {
                            state.list_state.select(Some(state.filtered.len() - 1));
                        }
                    }
                },

                KeyCode::Down => match state.list_state.selected() {
                    Some(selected) => {
                        if selected + 1 < state.filtered.len() {
                            state.list_state.select(Some(selected + 1));
                        }
                    }

                    None => {
                        if !state.filtered.is_empty() {
                            state.list_state.select(Some(0));
                        }
                    }
                },

                _ => {
                    state.input_widget.handle_event(&Event::Key(key));
                }
            }
        }
    }
}

fn draw_ui<T: Clone>(f: &mut Frame, state: &mut State<T>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(10)])
        .split(f.size());

    // === Draw input line === //

    let scroll = state.input_widget.visual_scroll(
        (
            // Keep 1 space for cursor
            chunks[0].width.max(1) - 1
        ) as usize,
    );

    let input = Paragraph::new(state.input_widget.value()).scroll((0, scroll as u16));

    f.render_widget(input, chunks[0]);

    f.set_cursor(
        chunks[0].x + (state.input_widget.visual_cursor().max(scroll) - scroll) as u16,
        chunks[0].y,
    );

    // === Draw results list === //

    let results = state
        .filtered
        .iter()
        .cloned()
        .map(|item| ListItem::new(item.display))
        .collect::<Vec<_>>();

    let results = List::new(results).highlight_style(Style::default().bg(Color::Black));

    f.render_stateful_widget(results, chunks[1], &mut state.list_state);
}

fn fuzzy_find_match<T: Clone>(query: &str, list: &[FuzzyFinderItem<T>]) -> Vec<FuzzyFinderItem<T>> {
    if query.is_empty() {
        return list.to_vec();
    }

    let mut scores = list
        .iter()
        .enumerate()
        .map(|(i, item)| (i, compute_fuzzy_find_score(query, &item.display)))
        .filter(|(_, score)| *score > 0)
        .collect::<Vec<_>>();

    scores.sort_by_key(|(_, score)| *score);

    scores
        .into_iter()
        .map(|(i, _)| list.get(i).unwrap())
        .rev()
        .cloned()
        .collect()
}

fn compute_fuzzy_find_score(query: &str, subject: &str) -> usize {
    query
        .split_ascii_whitespace()
        .filter_map(|word| {
            if subject.contains(word) {
                Some(word.chars().count())
            } else {
                None
            }
        })
        .sum()
}

struct State<T: Clone> {
    input_widget: Input,
    list: Vec<FuzzyFinderItem<T>>,
    list_state: ListState,
    filtered: Vec<FuzzyFinderItem<T>>,
}
