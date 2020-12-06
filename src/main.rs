#![no_std]
#![no_main]

// pick a panicking behavior
//use panic_halt as _; // you can put a breakpoint on `rust_begin_unwind` to catch panics

// use panic_abort as _; // requires nightly
// use panic_itm as _; // logs messages over ITM; requires ITM support
// use panic_semihosting as _; // logs messages to the host stderr; requires a debugger

use crate::hal::{pac, prelude::*};
use core::panic::PanicInfo;
use cortex_m_rt::entry;
use pac::interrupt;
use rtt_target::{rprintln, rtt_init_print};
use stm32f4xx_hal as hal;

fn i2s_sr_check() {
    unsafe {
        let spi2 = &(*pac::SPI2::ptr());
        if spi2.sr.read().fre().bit() {
            rprintln!("Frame Error");
        }
        if spi2.sr.read().ovr().bit() {
            rprintln!("Overrun");
        }
        if spi2.sr.read().udr().bit() {
            rprintln!("underrun");
        }
        if !spi2.sr.read().txe().bit() {
            rprintln!("buffer not empty");
        }
    }
}

const MCK_USE: bool = false;

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let device = pac::Peripherals::take().unwrap();
    let gpiob = device.GPIOB.split();
    let gpioc = device.GPIOC.split();
    let rcc = device.RCC.constrain();
    let mut data = [0i32; 480];
    //build a sawtooth period
    for (i, e) in data.iter_mut().enumerate() {
        *e = i32::MIN / 2 + (i as i32 * ((u32::MAX) / 480 / 2) as i32);
    }
    let _clocks = rcc
        .cfgr
        .use_hse(8.mhz())
        .sysclk(96.mhz())
        .hclk(96.mhz())
        .pclk1(50.mhz())
        .pclk2(100.mhz())
        .freeze();
    //enable system clock for APB bus SPI2 and SPI5
    unsafe {
        let rcc = &(*pac::RCC::ptr());
        rcc.apb1enr
            .modify(|_, w| w.pwren().set_bit().spi2en().set_bit());
        rcc.apb2enr.modify(|_, w| w.spi5en().set_bit());
    }

    //setup  and startup common i2s clock
    unsafe {
        let rcc = &(*pac::RCC::ptr());
        //setup
        rcc.plli2scfgr.modify(|_, w| {
            if MCK_USE {
                w.plli2sr().bits(5).plli2sn().bits(192).plli2sm().bits(5)
            } else {
                w.plli2sr().bits(5).plli2sn().bits(192).plli2sm().bits(4)
            }
        });
        //run the clock
        rcc.cr.modify(|_, w| w.plli2son().set_bit());
        //wait a stable clock
        while rcc.cr.read().plli2srdy().bit_is_clear() {}
    }
    //i2s2 gpio
    //  CK pb10,pb13,pc7,*pd3
    //  SD pb15,pc3
    //  WS pb9, pb12
    //  MCK pa3, pa6, pc6,

    let _pb13 = gpiob.pb13.into_alternate_af5(); //CK BCK
    let _pb15 = gpiob.pb15.into_alternate_af5(); //SD DIN
    let _pb12 = gpiob.pb12.into_alternate_af5(); //WS LRCK
    let _pc6 = gpioc.pc6.into_alternate_af5(); //MCK SCK

    //i2s5 gpio
    // CK pb0
    // SD pa10, pb8
    // WS pb1
    let _pb0 = gpiob.pb0.into_alternate_af6(); //CK BCK
    let _pb8 = gpiob.pb8.into_alternate_af6(); //SD DIN
    let _pb1 = gpiob.pb1.into_alternate_af6(); //WS LRCK

    //i2s2 interrupt
    unsafe {
        let spi2 = &(*pac::SPI2::ptr());
        spi2.cr2
            .modify(|_, w| w.txeie().clear_bit().rxneie().clear_bit().errie().set_bit());
        pac::NVIC::unmask(pac::Interrupt::SPI2);
    }

    //i2s5 interrupt
    unsafe {
        let spi5 = &(*pac::SPI5::ptr());
        spi5.cr2
            .modify(|_, w| w.txeie().clear_bit().rxneie().set_bit().errie().set_bit());
        pac::NVIC::unmask(pac::Interrupt::SPI5);
    }

    //Spi2 setup for i2s mode
    unsafe {
        let spi2 = &(*pac::SPI2::ptr());
        spi2.i2spr.modify(|_, w| {
            if MCK_USE {
                w.i2sdiv().bits(2).odd().set_bit().mckoe().enabled()
            } else {
                w.i2sdiv().bits(12).odd().set_bit().mckoe().disabled()
            }
        });
        spi2.i2scfgr.modify(|_, w| {
            w.i2smod()
                .i2smode() //
                .i2scfg()
                .master_tx() //
                .pcmsync()
                .long() //
                .i2sstd()
                .philips() //
                .ckpol()
                .idle_high() //
                .datlen()
                .sixteen_bit() //
                .chlen()
                .thirty_two_bit() //
        })
    }

    //Spi5 setup for i2s mode
    unsafe {
        let spi5 = &(*pac::SPI5::ptr());
        spi5.i2spr.modify(|_, w| {
            if MCK_USE {
                w.i2sdiv().bits(2).odd().set_bit().mckoe().enabled()
            } else {
                w.i2sdiv().bits(12).odd().set_bit().mckoe().disabled()
            }
        });
        spi5.i2scfgr.modify(|_, w| {
            w.i2smod()
                .i2smode() //
                .i2scfg()
                .slave_rx() //
                .pcmsync()
                .long() //
                .i2sstd()
                .philips() //
                .ckpol()
                .idle_high() //
                .datlen()
                .sixteen_bit() //
                .chlen()
                .thirty_two_bit() //
        })
    }

    //enable i2s5 and then i2s2
    unsafe {
        let spi5 = &(*pac::SPI5::ptr());
        spi5.i2scfgr.modify(|_, w| w.i2se().enabled());
        let spi2 = &(*pac::SPI2::ptr());
        spi2.i2scfgr.modify(|_, w| w.i2se().enabled());
    }

    rprintln!("init done");
    //check spi2 status
    unsafe {
        let spi2 = &(*pac::SPI2::ptr());
        let spi2_sr = *((pac::SPI2::ptr() as usize + 0x08) as *const u32);
        rprintln!("{:#032b} {}", spi2_sr, spi2.sr.read().txe().bit());
    }

    //let mut data_iter = data.iter().cycle();

    loop {
        //if let Some(data) = data_iter.next() {
        //    let data = *data as u32;
        //    let l = data;
        //    let r = data;

        //    unsafe {
        //        let spi2 = &(*pac::SPI2::ptr());
        //        while !spi2.sr.read().txe().bit() {}
        //        spi2.dr.modify(|_, w| w.dr().bits((l >> 16) as u16));
        //        i2s_sr_check();
        //        while !spi2.sr.read().txe().bit() {}
        //        spi2.dr.modify(|_, w| w.dr().bits((l & 0x00FF) as u16));
        //        i2s_sr_check();
        //        while !spi2.sr.read().txe().bit() {}
        //        spi2.dr.modify(|_, w| w.dr().bits((r >> 16) as u16));
        //        i2s_sr_check();
        //        while !spi2.sr.read().txe().bit() {}
        //        spi2.dr.modify(|_, w| w.dr().bits((r & 0x00FF) as u16));
        //        i2s_sr_check();
        //    }
        //}
        unsafe {
            let spi2 = &(*pac::SPI2::ptr());
            while !spi2.sr.read().txe().bit() {}
            spi2.dr.modify(|_, w| w.dr().bits(0xFEED));
        }
    }
}

#[interrupt]
fn SPI2() {
    unsafe {
        let spi2 = &(*pac::SPI2::ptr());
        if spi2.sr.read().fre().bit() {
            rprintln!("Frame Error");
        }
        if spi2.sr.read().ovr().bit() {
            rprintln!("Overrun");
        }
        if spi2.sr.read().udr().bit() {
            rprintln!("underrun");
        }
    }
}

#[interrupt]
fn SPI5() {
    unsafe {
        let spi5 = &(*pac::SPI5::ptr());
        if spi5.sr.read().fre().bit() {
            rprintln!("SPI5 Frame Error");
        }
        if spi5.sr.read().ovr().bit() {
            rprintln!("SPI5 Overrun");
        }
        if spi5.sr.read().udr().bit() {
            rprintln!("SPI5 underrun");
        }
        if spi5.sr.read().rxne().bit() {
            let data = spi5.dr.read().dr().bits();
            rprintln!("SPI5 rx {:#04X}",data);
        }
    }
}

#[inline(never)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rprintln!("{}", info);
    loop {} // You might need a compiler fence in here.
}
