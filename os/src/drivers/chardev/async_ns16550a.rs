use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use core::task::Poll::{Pending, Ready};

use bitflags::*;
use volatile::{ReadOnly, Volatile, WriteOnly};

use crate::sync::UPIntrFreeCell;

bitflags! {
    /// InterruptEnableRegister
    pub struct IER: u8 {
        const RX_AVAILABLE = 1 << 0;
        const TX_EMPTY = 1 << 1;
    }

    /// LineStatusRegister
    pub struct LSR: u8 {
        const DATA_AVAILABLE = 1 << 0;
        const THR_EMPTY = 1 << 5;
    }

    /// Model Control Register
    pub struct MCR: u8 {
        const DATA_TERMINAL_READY = 1 << 0;
        const REQUEST_TO_SEND = 1 << 1;
        const AUX_OUTPUT1 = 1 << 2;
        const AUX_OUTPUT2 = 1 << 3;
    }
}

#[repr(C)]
#[allow(dead_code)]
struct ReadWithoutDLAB {
    /// receiver buffer register
    pub rbr: ReadOnly<u8>,
    /// interrupt enable register
    pub ier: Volatile<IER>,
    /// interrupt identification register
    pub iir: ReadOnly<u8>,
    /// line control register
    pub lcr: Volatile<u8>,
    /// model control register
    pub mcr: Volatile<MCR>,
    /// line status register
    pub lsr: ReadOnly<LSR>,
    /// ignore MSR
    _padding1: ReadOnly<u8>,
    /// ignore SCR
    _padding2: ReadOnly<u8>,
}

#[repr(C)]
#[allow(dead_code)]
struct WriteWithoutDLAB {
    /// transmitter holding register
    pub thr: WriteOnly<u8>,
    /// interrupt enable register
    pub ier: Volatile<IER>,
    /// ignore FCR
    _padding0: ReadOnly<u8>,
    /// line control register
    pub lcr: Volatile<u8>,
    /// modem control register
    pub mcr: Volatile<MCR>,
    /// line status register
    pub lsr: ReadOnly<LSR>,
    /// ignore other registers
    _padding1: ReadOnly<u16>,
}

///! Ref: https://www.lammertbies.nl/comm/info/serial-uart
///! Ref: ns16550a datasheet: https://datasheetspdf.com/pdf-file/605590/NationalSemiconductor/NS16550A/1
///! Ref: ns16450 datasheet: https://datasheetspdf.com/pdf-file/1311818/NationalSemiconductor/NS16450/1
pub struct NS16550aRaw {
    base_addr: usize,
    read_waker_list: VecDeque<Waker>,
    write_waker_list: VecDeque<Waker>,
    read_buffer: VecDeque<u8>,
}

impl NS16550aRaw {
    fn read_end(&mut self) -> &mut ReadWithoutDLAB {
        unsafe { &mut *(self.base_addr as *mut ReadWithoutDLAB) }
    }

    fn write_end(&mut self) -> &mut WriteWithoutDLAB {
        unsafe { &mut *(self.base_addr as *mut WriteWithoutDLAB) }
    }

    pub fn new(base_addr: usize) -> Self {
        Self { base_addr,
            read_buffer: VecDeque::new(),
            read_waker_list: VecDeque::new(),
            write_waker_list: VecDeque::new(),
        }
    }

    pub fn init(&mut self) {
        let read_end = self.read_end();
        let mut mcr = MCR::empty();
        mcr |= MCR::DATA_TERMINAL_READY;
        mcr |= MCR::REQUEST_TO_SEND;
        mcr |= MCR::AUX_OUTPUT2;
        read_end.mcr.write(mcr);
        let ier = IER::RX_AVAILABLE;
        read_end.ier.write(ier);
    }

    pub fn read(&mut self) -> Option<u8> {
        let read_end = self.read_end();
        let lsr = read_end.lsr.read();
        if lsr.contains(LSR::DATA_AVAILABLE) {
            Some(read_end.rbr.read())
        } else {
            None
        }
    }

    pub fn writable(&mut self) -> bool {
        let write_end = self.write_end();
        write_end.lsr.read().contains(LSR::THR_EMPTY)
    }
}

pub struct AsyncNS16550a<const BASE_ADDR: usize> {
    inner: Arc<UPIntrFreeCell<NS16550aRaw>>,
}

impl<const BASE_ADDR: usize> AsyncNS16550a<BASE_ADDR> {
    pub fn new() -> Self {
        let inner = NS16550aRaw::new(BASE_ADDR);
        //inner.ns16550a.init();
        Self {
            inner: Arc::new(unsafe { UPIntrFreeCell::new(inner) }),
        }
    }
    pub fn init(&self) {
        let inner = self.inner.clone();
        inner.exclusive_access().init();
        drop(inner);
    }

    pub fn read(self: Arc<Self>) -> AsyncCharReader<BASE_ADDR> {
        AsyncCharReader { ns16550a: self }
    }
    pub fn write(self: Arc<Self>, ch: u8) -> AsyncCharWriter<BASE_ADDR> {
        AsyncCharWriter { ns16550a: self, ch }
    }

    pub fn handle_irq(&self) {
        self.inner.clone().exclusive_session(|inner| {
            if let Some(ch) = inner.read() {
                if let Some(waker) = inner.read_waker_list.pop_front() {
                    inner.read_buffer.push_back(ch);
                    waker.clone().wake();
                }
            }

            if inner.writable() {
                if let Some(waker) = inner.write_waker_list.pop_front() {
                    waker.clone().wake();
                }
            }
        });
    }
}


pub struct AsyncCharWriter<const BASE_ADDR: usize> {
    ns16550a: Arc<AsyncNS16550a<BASE_ADDR>>,
    ch: u8,
}

impl<const BASE_ADDR: usize> Future for AsyncCharWriter<BASE_ADDR> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut raw = self.ns16550a.inner.exclusive_access();
        let write_end = raw.write_end();
        if write_end.lsr.read().contains(LSR::THR_EMPTY) {
            // writable
            write_end.thr.write(self.ch);
            Ready(())
        } else {
            let waker = cx.waker().clone();
            raw.write_waker_list.push_back(waker);
            Pending
        }
    }
}

pub struct AsyncCharReader<const BASE_ADDR: usize> {
    ns16550a: Arc<AsyncNS16550a<BASE_ADDR>>,
}

impl<const BASE_ADDR: usize> Future for AsyncCharReader<BASE_ADDR> {
    type Output = u8;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let clone = self.ns16550a.clone();
        let mut raw = clone.inner.exclusive_access();
        if let Some(ch) = raw.read_buffer.pop_front() {
            // readable
            drop(raw);
            Ready(ch)
        } else {
            let waker = cx.waker().clone();
            let will_wake = raw.read_waker_list.iter()
                .any(|x| x.will_wake(&waker));
            if !will_wake {
                raw.read_waker_list.push_back(waker);
                drop(raw);
            }
            return Pending;
        }
    }
}

