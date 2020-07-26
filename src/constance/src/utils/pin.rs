use core::pin::Pin;

pub fn static_pin<T: ?Sized>(x: &'static T) -> Pin<&'static T> {
    // Safety: The pointee will never disappear without calling the destructor
    unsafe { Pin::new_unchecked(x) }
}
