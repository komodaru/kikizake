// A slot of the case language is not the object once made there (the review of 8480456, 2.1:
// a view kept the slot number): a Slot where an Id goes does not build.
use kikizake::model::objects::{Objects, Slot};

fn main() {
    let o: Objects<u32> = Objects::default();
    if let Ok(s) = Slot::<u32>::new(0) {
        let _ = o.get(s);
    }
}
