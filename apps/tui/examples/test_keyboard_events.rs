use crossterm::event::{self, Event, KeyCode};
use std::io;
use std::time::Duration;

fn main() -> io::Result<()> {
    println!("Testing keyboard event capture. Press keys to test (Ctrl+C to exit):");
    
    crossterm::terminal::enable_raw_mode()?;
    
    loop {
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()? {
                println!("Key pressed: {:?}", key);
                
                match key.code {
                    KeyCode::Up => println!("  -> Up arrow detected!"),
                    KeyCode::Down => println!("  -> Down arrow detected!"),
                    KeyCode::Left => println!("  -> Left arrow detected!"),
                    KeyCode::Right => println!("  -> Right arrow detected!"),
                    KeyCode::Enter => println!("  -> Enter key detected!"),
                    KeyCode::Esc => {
                        println!("  -> Escape key detected! Exiting...");
                        break;
                    }
                    KeyCode::Char('q') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                        println!("  -> Ctrl+Q detected! Exiting...");
                        break;
                    }
                    KeyCode::Char(c) => println!("  -> Character '{}' detected!", c),
                    _ => {}
                }
            }
    }
    
    crossterm::terminal::disable_raw_mode()?;
    println!("Test completed.");
    
    Ok(())
}