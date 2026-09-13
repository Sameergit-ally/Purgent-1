use purgent_core::modules::storage;

fn main() {
    for device in storage::list_devices() {
        println!("{device:#?}");
    }
}
