use std::mem::ManuallyDrop;

pub trait VecExt<T> {
    fn into_raw_parts_(self) -> (*mut T, usize, usize);
}

impl<T> VecExt<T> for Vec<T> {
    fn into_raw_parts_(self) -> (*mut T, usize, usize) {
        let mut me = ManuallyDrop::new(self);
        (me.as_mut_ptr(), me.len(), me.capacity())
    }
}
