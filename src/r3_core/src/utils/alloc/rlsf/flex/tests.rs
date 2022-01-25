use quickcheck_macros::quickcheck;
use std::prelude::v1::*;

use super::super::{
    tests::ShadowAllocator,
    utils::{self, nonnull_slice_end, nonnull_slice_len},
};
use super::*;

trait TestFlexSource: FlexSource {
    type Options: quickcheck::Arbitrary;

    fn new(options: Self::Options) -> Self;
}

impl<T: core::alloc::GlobalAlloc + Default, const ALIGN: usize> TestFlexSource
    for GlobalAllocAsFlexSource<T, ALIGN>
{
    type Options = ();

    fn new((): ()) -> Self {
        Self(T::default())
    }
}

#[derive(Debug)]
struct TrackingFlexSource<T: FlexSource> {
    sa: ShadowAllocator,
    inner: T,
}

impl<T: TestFlexSource> TestFlexSource for TrackingFlexSource<T> {
    type Options = T::Options;

    fn new(options: T::Options) -> Self {
        Self {
            sa: ShadowAllocator::default(),
            inner: T::new(options),
        }
    }
}

impl<T: FlexSource> Drop for TrackingFlexSource<T> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            return;
        }

        if self.inner.supports_dealloc() {
            // All existing pools should have been removed by `FlexTlsf::drop`
            self.sa.assert_no_pools();
        }
    }
}

unsafe impl<T: FlexSource> FlexSource for TrackingFlexSource<T> {
    unsafe fn alloc(&mut self, min_size: usize) -> Option<NonNull<[u8]>> {
        log::trace!("FlexSource::alloc({:?})", min_size);
        let range = self.inner.alloc(min_size)?;
        log::trace!(" FlexSource::alloc(...) = {:?}", range);
        self.sa.insert_free_block(range.as_ptr());
        Some(range)
    }

    unsafe fn realloc_inplace_grow(
        &mut self,
        ptr: NonNull<[u8]>,
        min_new_len: usize,
    ) -> Option<usize> {
        log::trace!("FlexSource::realloc_inplace_grow{:?}", (ptr, min_new_len));
        let new_len = self.inner.realloc_inplace_grow(ptr, min_new_len)?;
        log::trace!(" FlexSource::realloc_inplace_grow(...) = {:?}", new_len);
        self.sa.append_free_block(std::ptr::slice_from_raw_parts(
            nonnull_slice_end(ptr),
            new_len - nonnull_slice_len(ptr),
        ));
        Some(new_len)
    }

    #[inline]
    fn min_align(&self) -> usize {
        self.inner.min_align()
    }

    #[inline]
    unsafe fn dealloc(&mut self, ptr: NonNull<[u8]>) {
        // TODO: check that `ptr` represents an exact allocation, not just
        //       a part of it
        self.inner.dealloc(ptr);
        log::trace!("FlexSource::dealloc({:?})", ptr);
        self.sa.remove_pool(ptr.as_ptr());
    }

    #[inline]
    fn is_contiguous_growable(&self) -> bool {
        self.inner.is_contiguous_growable()
    }

    #[inline]
    fn supports_dealloc(&self) -> bool {
        self.inner.supports_dealloc()
    }

    #[inline]
    fn supports_realloc_inplace_grow(&self) -> bool {
        self.inner.supports_realloc_inplace_grow()
    }
}

fn fill_data(p: NonNull<[u8]>) {
    use std::mem::MaybeUninit;
    let slice = unsafe { &mut *(p.as_ptr() as *mut [MaybeUninit<u8>]) };
    for (i, p) in slice.iter_mut().enumerate() {
        *p = MaybeUninit::new((i as u8).reverse_bits());
    }
}

fn verify_data(p: NonNull<[u8]>) {
    let slice = unsafe { p.as_ref() };
    for (i, p) in slice.iter().enumerate() {
        assert_eq!(*p, (i as u8).reverse_bits());
    }
}

macro_rules! gen_test {
    ($mod:ident, $source:ty, $($tt:tt)*) => {
        mod $mod {
            use super::*;
            type TheTlsf = FlexTlsf<TrackingFlexSource<$source>, $($tt)*>;

            #[quickcheck]
            fn minimal(source_options: <$source as TestFlexSource>::Options) {
                let _ = env_logger::builder().is_test(true).try_init();

                let mut tlsf = TheTlsf::new(TrackingFlexSource::new(source_options));

                log::trace!("tlsf = {:?}", tlsf);

                let ptr = tlsf.allocate(Layout::from_size_align(1, 1).unwrap());
                log::trace!("ptr = {:?}", ptr);
                if let Some(ptr) = ptr {
                    unsafe { tlsf.deallocate(ptr, 1) };
                }

                tlsf.destroy();
            }

            #[quickcheck]
            fn random(source_options: <$source as TestFlexSource>::Options, max_alloc_size: usize, bytecode: Vec<u8>) {
                random_inner(source_options, max_alloc_size, bytecode);
            }

            struct CleanOnDrop(Option<TheTlsf>);

            impl Drop for CleanOnDrop {
                fn drop(&mut self) {
                    self.0.take().unwrap().destroy();
                }
            }

            fn random_inner(source_options: <$source as TestFlexSource>::Options, max_alloc_size: usize, bytecode: Vec<u8>) -> Option<()> {
                let max_alloc_size = max_alloc_size % 0x10000;

                let mut tlsf = CleanOnDrop(None);
                let tlsf = tlsf.0.get_or_insert_with(|| TheTlsf::new(TrackingFlexSource::new(source_options)));
                macro_rules! sa {
                    () => {
                        unsafe { tlsf.source_mut_unchecked() }.sa
                    };
                }

                log::trace!("tlsf = {:?}", tlsf);

                #[derive(Debug)]
                struct Alloc {
                    ptr: NonNull<u8>,
                    layout: Layout,
                }
                let mut allocs = Vec::new();

                let mut it = bytecode.iter().cloned();
                loop {
                    match it.next()? % 8 {
                        0..=2 => {
                            let len = u32::from_le_bytes([
                                it.next()?,
                                it.next()?,
                                it.next()?,
                                0,
                            ]);
                            let len = ((len as u64 * max_alloc_size as u64) >> 24) as usize;
                            let align = 1 << (it.next()? % (ALIGN.trailing_zeros() as u8 + 1));
                            assert!(align <= ALIGN);
                            let layout = Layout::from_size_align(len, align).unwrap();
                            log::trace!("alloc {:?}", layout);

                            let ptr = tlsf.allocate(layout);
                            log::trace!(" → {:?}", ptr);

                            if let Some(ptr) = ptr {
                                allocs.push(Alloc { ptr, layout });
                                sa!().allocate(layout, ptr);

                                // Fill it with dummy data
                                fill_data(utils::nonnull_slice_from_raw_parts(ptr, len));
                            }
                        }
                        3..=5 => {
                            let alloc_i = it.next()?;
                            if allocs.len() > 0 {
                                let alloc = allocs.swap_remove(alloc_i as usize % allocs.len());
                                log::trace!("dealloc {:?}", alloc);

                                // Make sure the stored dummy data is not corrupted
                                verify_data(utils::nonnull_slice_from_raw_parts(alloc.ptr, alloc.layout.size()));

                                unsafe { tlsf.deallocate(alloc.ptr, alloc.layout.align()) };
                                sa!().deallocate(alloc.layout, alloc.ptr);
                            }
                        }
                        6..=7 => {
                            let alloc_i = it.next()?;
                            if allocs.len() > 0 {
                                let len = u32::from_le_bytes([
                                    it.next()?,
                                    it.next()?,
                                    it.next()?,
                                    0,
                                ]);
                                let len = ((len as u64 * max_alloc_size as u64) >> 24) as usize;

                                let alloc_i = alloc_i as usize % allocs.len();
                                let alloc = &mut allocs[alloc_i];
                                log::trace!("realloc {:?} to {:?}", alloc, len);

                                let new_layout = Layout::from_size_align(len, alloc.layout.align()).unwrap();

                                if let Some(ptr) = unsafe { tlsf.reallocate(alloc.ptr, new_layout) } {
                                    log::trace!(" {:?} → {:?}", alloc.ptr, ptr);

                                    // Check and refill the dummy data
                                    verify_data(utils::nonnull_slice_from_raw_parts(ptr, len.min(alloc.layout.size())));
                                    fill_data(utils::nonnull_slice_from_raw_parts(ptr, len));

                                    sa!().deallocate(alloc.layout, alloc.ptr);
                                    alloc.ptr = ptr;
                                    alloc.layout = new_layout;
                                    sa!().allocate(alloc.layout, alloc.ptr);
                                } else {
                                    log::trace!(" {:?} → fail", alloc.ptr);

                                }
                            }
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
    };
}

type SysSource = GlobalAllocAsFlexSource<std::alloc::System, 1024>;
gen_test!(tlsf_sys_u8_u8_1_1, SysSource, u8, u8, 1, 1);
gen_test!(tlsf_sys_u8_u8_1_2, SysSource, u8, u8, 1, 2);
gen_test!(tlsf_sys_u8_u8_1_4, SysSource, u8, u8, 1, 4);
gen_test!(tlsf_sys_u8_u8_1_8, SysSource, u8, u8, 1, 8);
gen_test!(tlsf_sys_u8_u8_3_4, SysSource, u8, u8, 3, 4);
gen_test!(tlsf_sys_u8_u8_5_4, SysSource, u8, u8, 5, 4);
gen_test!(tlsf_sys_u8_u8_8_1, SysSource, u8, u8, 8, 1);
gen_test!(tlsf_sys_u8_u8_8_8, SysSource, u8, u8, 8, 8);
gen_test!(tlsf_sys_u16_u8_3_4, SysSource, u16, u8, 3, 4);
gen_test!(tlsf_sys_u16_u8_11_4, SysSource, u16, u8, 11, 4);
gen_test!(tlsf_sys_u16_u8_16_4, SysSource, u16, u8, 16, 4);
gen_test!(tlsf_sys_u16_u16_3_16, SysSource, u16, u16, 3, 16);
gen_test!(tlsf_sys_u16_u16_11_16, SysSource, u16, u16, 11, 16);
gen_test!(tlsf_sys_u16_u16_16_16, SysSource, u16, u16, 16, 16);
gen_test!(tlsf_sys_u16_u32_3_16, SysSource, u16, u32, 3, 16);
gen_test!(tlsf_sys_u16_u32_11_16, SysSource, u16, u32, 11, 16);
gen_test!(tlsf_sys_u16_u32_16_16, SysSource, u16, u32, 16, 16);
gen_test!(tlsf_sys_u16_u32_3_32, SysSource, u16, u32, 3, 32);
gen_test!(tlsf_sys_u16_u32_11_32, SysSource, u16, u32, 11, 32);
gen_test!(tlsf_sys_u16_u32_16_32, SysSource, u16, u32, 16, 32);
gen_test!(tlsf_sys_u32_u32_20_32, SysSource, u32, u32, 20, 32);
gen_test!(tlsf_sys_u32_u32_27_32, SysSource, u32, u32, 27, 32);
gen_test!(tlsf_sys_u32_u32_28_32, SysSource, u32, u32, 28, 32);
gen_test!(tlsf_sys_u32_u32_29_32, SysSource, u32, u32, 29, 32);
gen_test!(tlsf_sys_u32_u32_32_32, SysSource, u32, u32, 32, 32);
gen_test!(tlsf_sys_u64_u8_58_8, SysSource, u64, u64, 58, 8);
gen_test!(tlsf_sys_u64_u8_59_8, SysSource, u64, u64, 59, 8);
gen_test!(tlsf_sys_u64_u8_60_8, SysSource, u64, u64, 60, 8);
gen_test!(tlsf_sys_u64_u8_61_8, SysSource, u64, u64, 61, 8);
gen_test!(tlsf_sys_u64_u8_64_8, SysSource, u64, u64, 64, 8);
