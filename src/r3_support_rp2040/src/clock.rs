#![cfg(feature = "semver-exempt")]

/// Configure the clocks.
///
///  - The crystal oscillator frequency is assumed to be 12MHz (Raspberry Pi
///    Pico).
///  - `clk_ref` = 48MHz
///  - `clk_sys` = 125MHz
///  - Watchdog ticks = 1MHz
///
pub fn init_clock(
    clocks: &rp2040::CLOCKS,
    xosc: &rp2040::XOSC,
    pll_sys: &rp2040::PLL_SYS,
    pll_usb: &rp2040::PLL_USB,
    resets: &rp2040::RESETS,
    watchdog: &rp2040::WATCHDOG,
) {
    // Disable Resus
    clocks.clk_sys_resus_ctrl.write(|b| b.enable().clear_bit());

    // Switch them away from PLL
    clocks.clk_ref_ctrl.modify(|_, w| w.src().rosc_clksrc_ph());
    clocks.clk_sys_ctrl.modify(|_, w| w.src().clk_ref());
    while clocks.clk_ref_selected.read().bits() != 1 {}
    while clocks.clk_sys_selected.read().bits() != 1 {}

    // Reset PLL
    resets
        .reset
        .modify(|_, w| w.pll_sys().set_bit().pll_usb().set_bit());
    resets
        .reset
        .modify(|_, w| w.pll_sys().clear_bit().pll_usb().clear_bit());
    while resets.reset_done.read().pll_sys().bit_is_clear() {}
    while resets.reset_done.read().pll_usb().bit_is_clear() {}

    // Raspberry Pi Pico on-board PLL
    const MHZ: u32 = 12;

    // Assumes 1-15 MHz input
    assert!(MHZ >= 1 && MHZ <= 15);
    xosc.ctrl.write(|b| b.freq_range()._1_15mhz());

    // Set XOSC startup delay
    xosc.startup
        .write(|b| unsafe { b.bits((MHZ * 1000 + 128) / 256) });

    // Enable XOSC
    xosc.ctrl.modify(|_, w| w.enable().enable());

    // Wait for stabilization, like we always do
    while xosc.status.read().stable().bit_is_clear() {}

    macro_rules! cfg_pll {
        ($pll:ident = MHZ mhz * $fbdiv:literal / $post_div1:literal / $post_div2:literal) => {
            // Turn off the PLL
            $pll.pwr.write(|b| {
                b.vcopd()
                    .set_bit()
                    .postdivpd()
                    .set_bit()
                    .dsmpd()
                    .set_bit()
                    .pd()
                    .set_bit()
            });

            // Configure the reference divider
            $pll.cs.write(|b| unsafe { b.refdiv().bits(1) });

            // Configure the feedback divider
            $pll.fbdiv_int
                .write(|b| unsafe { b.fbdiv_int().bits($fbdiv) });

            // Turn on the PLL
            $pll.pwr.modify(|_, w| {
                w.vcopd() // VCO
                    .clear_bit()
                    .pd() // main power
                    .clear_bit()
            });

            // Wait for the PLL to lock
            while $pll.cs.read().lock().bit_is_clear() {}

            // Configure the post dividers
            $pll.prim
                .write(|b| unsafe { b.postdiv1().bits($post_div1).postdiv2().bits($post_div2) });

            // Turn on the post dividers
            $pll.pwr.modify(|_, w| w.postdivpd().clear_bit());
        };
    }
    cfg_pll!(pll_sys = MHZ mhz * 125 / 6 / 2);
    cfg_pll!(pll_usb = MHZ mhz * 40 / 5 / 2);

    // pll_sys → clk_sys
    clocks
        .clk_sys_ctrl
        .modify(|_, w| w.auxsrc().clksrc_pll_sys());
    clocks
        .clk_sys_ctrl
        .modify(|_, w| w.src().clksrc_clk_sys_aux());

    // pll_usb → clk_ref
    clocks
        .clk_ref_ctrl
        .modify(|_, w| w.auxsrc().clksrc_pll_usb());
    clocks
        .clk_ref_ctrl
        .modify(|_, w| w.src().clksrc_clk_ref_aux());

    // Supply clk_ref / 48 = 1MHz to SysTick and watchdog
    watchdog.tick.write(|b| unsafe { b.cycles().bits(48) });

    // pll_usb → clk_peri
    clocks
        .clk_peri_ctrl
        .write(|b| b.auxsrc().clksrc_pll_usb().enable().set_bit());
}
