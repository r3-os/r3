/// Generate entry points for [`::cortex_m_rt`]. **Requires [`EntryPoint`]
/// and [`KernelTraits`] to be implemented.**
///
/// [`EntryPoint`]: crate::EntryPoint
/// [`KernelTraits`]: r3_kernel::KernelTraits
///
/// This macro registers the following items:
///
///  - The entry function (`#[cortex_m_rt::entry]`).
///  - The SysTick handler (`SysTick` global symbol).
///  - The PendSV handler (`PendSV` global symbol).
///  - Interrupt handlers and the vector table (`__INTERRUPTS` global symbol).
///
#[macro_export]
macro_rules! use_rt {
    (unsafe $Traits:ty) => {
        const _: () = {
            use $crate::{
                r3_kernel::KernelCfg2, rt::imp::ExceptionTrampoline, EntryPoint, INTERRUPT_SYSTICK,
            };

            #[link_section = ".vector_table.interrupts"]
            #[no_mangle]
            static __INTERRUPTS: $crate::rt::imp::InterruptHandlerTable =
                $crate::rt::imp::make_interrupt_handler_table::<$Traits>();

            #[$crate::cortex_m_rt::entry]
            fn main() -> ! {
                // Register `HANDLE_PEND_SV` as the PendSV handler under `cortex_m_rt`'s regime.
                //
                // `PEND_SV_TRAMPOLINE` contains the trampoline code. However, since it's not
                // recognized the linker as a Thumb function, its address does not have its least
                // significant bit set to mark a Thumb function. So we set the bit here.
                unsafe {
                    ::core::arch::asm!(
                        "
                            .global PendSV
                            PendSV = {} + 1
                        ",
                        sym PEND_SV_TRAMPOLINE
                    );
                }

                // `<$Traits as EntryPoint>::HANDLE_PEND_SV` contains the address of the PendSV
                // handler. Ideally we would like to simply assign the symbol address like the
                // following:
                //
                //     asm!(".global PendSV\n PendSV = {}", const HANDLE_PEND_SV);
                //
                // However, this does not work because `const` inputs do not currently accept
                // function pointers. So we assemble a trampoline function using a carefully
                // laid out `struct`.  The outcome is something like this:
                //
                //     .global PEND_SV_TRAMPOLINE
                //     PEND_SV_TRAMPOLINE:
                //         ldr pc, =(value of HANDLE_PEND_SV)
                //
                #[link_section = ".text"]
                static PEND_SV_TRAMPOLINE: ExceptionTrampoline =
                    ExceptionTrampoline::new(<$Traits as EntryPoint>::HANDLE_PEND_SV);

                unsafe { <$Traits as EntryPoint>::start() };
            }

            #[$crate::cortex_m_rt::exception]
            fn SysTick() {
                if let Some(x) = <$Traits as KernelCfg2>::INTERRUPT_HANDLERS.get(INTERRUPT_SYSTICK)
                {
                    // Safety: It's a first-level interrupt handler here. CPU Lock inactive
                    unsafe { x() };
                }
            }
        };
    };
}
