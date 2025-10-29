use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct NetSchemaContext {
    /// class_id (u32) -> network_name (e.g. "C_BasePlayerController")
    pub class_names: HashMap<u32, String>,
}

impl NetSchemaContext {
    pub fn insert_class(&mut self, id: u32, name: String) {
        self.class_names.insert(id, name);
    }
    pub fn name_of(&self, id: u32) -> Option<&str> {
        self.class_names.get(&id).map(|s| s.as_str())
    }
}