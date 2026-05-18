pub const DEFAULT_GROUP_ID: u32 = 0;
pub const DEFAULT_GROUP_NAME: &str = "Default";

#[derive(Clone, Debug)]
pub struct GroupTab {
    pub id: u32,
    pub name: String,
}

#[derive(Debug)]
pub struct GroupTabsState {
    pub user_groups: Vec<GroupTab>,
    pub selected_id: u32,
    next_group_number: u32,
    mru: Vec<u32>,
}

impl GroupTabsState {
    pub fn new() -> Self {
        Self {
            user_groups: Vec::new(),
            selected_id: DEFAULT_GROUP_ID,
            next_group_number: 1,
            mru: vec![DEFAULT_GROUP_ID],
        }
    }

    pub fn reset_for_new_folder(&mut self) {
        self.user_groups.clear();
        self.selected_id = DEFAULT_GROUP_ID;
        self.next_group_number = 1;
        self.mru.clear();
        self.mru.push(DEFAULT_GROUP_ID);
    }

    pub fn add_group(&mut self) -> u32 {
        let id = self.next_group_number;
        self.next_group_number += 1;

        let name = format!("Group {}", id);
        self.user_groups.push(GroupTab { id, name });

        self.mru.retain(|&value| value != id);
        self.mru.push(id);

        id
    }

    pub fn select_group(&mut self, id: u32) {
        if id != DEFAULT_GROUP_ID
            && !self.user_groups.iter().any(|group| group.id == id)
        {
            return;
        }

        self.selected_id = id;
        self.bump_mru(id);
    }

    pub fn close_group(&mut self, id: u32) {
        if id == DEFAULT_GROUP_ID {
            return;
        }

        self.user_groups.retain(|group| group.id != id);
        self.mru.retain(|&value| value != id);

        if self.selected_id == id {
            self.ensure_default_in_mru();
            let next_id = self.mru.first().copied().unwrap_or(DEFAULT_GROUP_ID);
            self.selected_id = next_id;
            self.bump_mru(next_id);
        }

        if self.user_groups.is_empty() {
            self.next_group_number = 1;
        }
    }

    pub fn is_selected(&self, id: u32) -> bool {
        self.selected_id == id
    }

    fn ensure_default_in_mru(&mut self) {
        if !self.mru.iter().any(|&value| value == DEFAULT_GROUP_ID) {
            self.mru.push(DEFAULT_GROUP_ID);
        }
    }

    fn bump_mru(&mut self, id: u32) {
        self.mru.retain(|&value| value != id);
        self.mru.insert(0, id);
    }
}
