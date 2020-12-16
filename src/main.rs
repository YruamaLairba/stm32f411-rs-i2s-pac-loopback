#![no_std]
#![no_main]

// pick a panicking behavior
//use panic_halt as _; // you can put a breakpoint on `rust_begin_unwind` to catch panics

// use panic_abort as _; // requires nightly
// use panic_itm as _; // logs messages over ITM; requires ITM support
// use panic_semihosting as _; // logs messages to the host stderr; requires a debugger

use crate::hal::{stm32, prelude::*};
use core::panic::PanicInfo;
use cortex_m_rt::entry;
use stm32::interrupt;
use rtt_target::{rprintln, rtt_init_print};
use stm32f4xx_hal as hal;

//PLLI2S clock configuration
const PLLI2SM: u8 = 2;
const PLLI2SN: u16 = 64;
const PLLI2SR: u8 = 4;

//Clock configuration of the used i2s interface
const I2SDIV: u8 = 62;
const ODD: bool = true;

//generate Master Clock ? Modifying this require to adapt the i2s clock
const MCK: bool = false;

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let device = stm32::Peripherals::take().unwrap();
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
        let rcc = &(*stm32::RCC::ptr());
        rcc.apb1enr
            .modify(|_, w| w.pwren().set_bit().spi2en().set_bit());
        rcc.apb2enr
            .modify(|_, w| w.spi5en().set_bit().syscfgen().set_bit());
    }

    //setup  and startup common i2s clock
    unsafe {
        let rcc = &(*stm32::RCC::ptr());
        //setup
        rcc.plli2scfgr.modify(|_, w| {
            w.plli2sr()
                .bits(PLLI2SR)
                .plli2sn()
                .bits(PLLI2SN)
                .plli2sm()
                .bits(PLLI2SM)
        });
        //run the clock
        rcc.cr.modify(|_, w| w.plli2son().set_bit());
        //wait a stable clock
        while rcc.cr.read().plli2srdy().bit_is_clear() {}
    }

    //i2s2 gpio
    //Note, on nucleo board possible i2s2 gpio are:
    //  CK: pb10, pb13, pc7
    //  SD: pb15, pc3
    //  WS: pb9, pb12
    //  MCK: pa3, pa6, pc6

    let _pb13 = gpiob.pb13.into_alternate_af5(); //CK
    let _pb15 = gpiob.pb15.into_alternate_af5(); //SD
    let _pb12 = gpiob.pb12.into_alternate_af5(); //WS
    let _pc6 = gpioc.pc6.into_alternate_af5(); //MCK

    //i2s5 gpio
    //Note, on nucleo board possible i2s5 gpio are:
    // CK pb0
    // SD pa10, pb8
    // WS pb1
    let _pb0 = gpiob.pb0.into_alternate_af6(); //CK BCK
    let _pb8 = gpiob.pb8.into_alternate_af6(); //SD DIN
    let mut _pb1 = gpiob.pb1.into_alternate_af6(); //WS LRCK

    //Setup an interrupt that can be triggered by pb1
    //Note: The hal doesn't allow to manipulate interrupt for pin in aternate mode
    unsafe {
        let syscfg = &(*stm32::SYSCFG::ptr());
        //EXTI0 interrupt on gpiob, pb0 to pb3 will trigger it
        syscfg.exticr1.modify(|_, w| w.exti0().bits(0b0001));
        let exti = &(*stm32::EXTI::ptr());
        //mask EXTI0 interrupt
        exti.imr.modify(|_, w| w.mr0().set_bit());
        //trigger interrupt on rising edge
        exti.rtsr.modify(|_, w| w.tr0().set_bit());
        //unmask EXTI0 interrupt
        stm32::NVIC::unmask(stm32::Interrupt::EXTI0);
    };

    //i2s2 interrupt
    unsafe {
        let spi2 = &(*stm32::SPI2::ptr());
        spi2.cr2
            .modify(|_, w| w.txeie().clear_bit().rxneie().clear_bit().errie().set_bit());
        stm32::NVIC::unmask(stm32::Interrupt::SPI2);
    }

    //i2s5 interrupt
    unsafe {
        let spi5 = &(*stm32::SPI5::ptr());
        spi5.cr2
            .modify(|_, w| w.txeie().clear_bit().rxneie().set_bit().errie().set_bit());
        stm32::NVIC::unmask(stm32::Interrupt::SPI5);
    }

    //Spi2 setup for i2s mode
    unsafe {
        let spi2 = &(*stm32::SPI2::ptr());
        spi2.i2spr
            .modify(|_, w| w.i2sdiv().bits(I2SDIV).odd().bit(ODD).mckoe().bit(MCK));
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
        let spi5 = &(*stm32::SPI5::ptr());
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
        let spi5 = &(*stm32::SPI5::ptr());
        spi5.i2scfgr.modify(|_, w| w.i2se().enabled());
        let spi2 = &(*stm32::SPI2::ptr());
        spi2.i2scfgr.modify(|_, w| w.i2se().enabled());
    }

    loop {
        unsafe {
            let spi2 = &(*stm32::SPI2::ptr());
            while !spi2.sr.read().txe().bit() {}
            if spi2.sr.read().chside().bit_is_clear() {
                spi2.dr.modify(|_, w| w.dr().bits(0b1111_1111_0000_0000));
            } else {
                spi2.dr.modify(|_, w| w.dr().bits(0b1111_1111_0000_0110));
            }
        }
    }
}

#[interrupt]
fn SPI2() {
    unsafe {
        let spi2 = &(*stm32::SPI2::ptr());
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
    static mut COUNT: u32 = 0;
    unsafe {
        let spi5 = &(*stm32::SPI5::ptr());
        if spi5.sr.read().fre().bit() {
            rprintln!("SPI5 Frame Error");
            //resynchronization
            spi5.i2scfgr.modify(|_, w| w.i2se().disabled());
            let gpiob = &(*stm32::GPIOB::ptr());
            let ws = gpiob.idr.read().idr1().bit();
            if ws {
                spi5.i2scfgr.modify(|_, w| w.i2se().enabled());
                rprintln!("Resynced (SPI5)");
            } else {
                let exti = &(*stm32::EXTI::ptr());
                exti.imr.modify(|_, w| w.mr0().set_bit());
            }
        } else if spi5.sr.read().ovr().bit() {
            rprintln!("SPI5 Overrun");
            //this sequence reset the interrupt
            let _ = spi5.dr.read().bits();
            let _ = spi5.sr.read().bits();
        } else if spi5.sr.read().udr().bit() {
            rprintln!("SPI5 underrun");
            //clear the interrupt
            let _ = spi5.sr.read().bits();
        } else if spi5.sr.read().rxne().bit() {
            let _data = spi5.dr.read().dr().bits();
            let _side = spi5.sr.read().chside().variant();
            *COUNT += 1;
            if *COUNT == 8_000 * 2 {
                rprintln!("8000 frame received");
                *COUNT = 0;
            }
        }
    }
}

#[interrupt]
fn EXTI0() {
    unsafe {
        let gpiob = &(*stm32::GPIOB::ptr());
        let ws = gpiob.idr.read().idr1().bit();
        let exti = &(*stm32::EXTI::ptr());
        //erase the event
        exti.pr.modify(|_, w| w.pr0().set_bit());
        //look if ws/pb1 is high
        if ws {
            //disable EXTI0 interrupt
            exti.imr.modify(|_, w| w.mr0().clear_bit());
            let spi5 = &(*stm32::SPI5::ptr());
            spi5.i2scfgr.modify(|_, w| w.i2se().enabled());
            rprintln!("Resynced (EXTI0)");
        }
    }
}

#[inline(never)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rprintln!("{}", info);
    loop {} // You might need a compiler fence in here.
}
