pub enum CurrentScreen {
    Main,
    Browsing,
    Editing,
    Exiting,
}

pub struct App {
    pub current_file: Option<String>,
    pub current_screen: CurrentScreen,
}

impl App {
    pub fn new() -> App {
        App {
            current_screen: CurrentScreen::Main,
            current_file: None,
        }
    }
}

