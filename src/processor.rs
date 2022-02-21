#![allow(dead_code)]

use crate::FONT_SET;

const CHIP8_OPCODE_SIZE: u16 = 2;
const CHIP8_FONT_SET_SIZE: usize = 80;
const CHIP8_RAM: usize = 4096;
const CHIP8_HEIGHT: usize = 32;
const CHIP8_WIDTH: usize = 64;
const CHIP8_NUM_REGS: usize = 16;

enum ProgramCounterAction {
    Skip,
    Next,
    Jump(u16),
}

impl ProgramCounterAction {
    fn skip_if(condition: bool) -> ProgramCounterAction {
        if condition {
            ProgramCounterAction::Skip
        } else {
            ProgramCounterAction::Next
        }
    }
}

pub struct Cpu {
    // RAM memory.
    ram: [u8; CHIP8_RAM],
    // Stack memory.
    stack: [u16; 16],
    // Program Counter.
    pc: u16,
    // Stack pointer.
    sp: u8,

    dt: u8,
    st: u8,

    // Index register.
    i: u16,
    // Registers array.
    v: [u8; CHIP8_NUM_REGS],
    // Graphics memory.
    vram: [[u8; CHIP8_WIDTH]; CHIP8_HEIGHT],
}

impl Cpu {
    pub fn new() -> Self {
        let mut ram = [0u8; CHIP8_RAM];

        // Load the font set into ram.
        for i in 0..CHIP8_FONT_SET_SIZE {
            ram[i] = FONT_SET[i];
        }

        Cpu {
            ram,
            pc: 0x200,
            vram: [[0; CHIP8_WIDTH]; CHIP8_HEIGHT],
            sp: 0,
            dt: 0,
            st: 0,
            i: 0,
            v: [0; CHIP8_NUM_REGS],
            stack: [0; 16],
        }
    }

    fn read_opcode(&self) -> u16 {
        let index = self.pc as usize;
        return ((self.ram[index] as u16) << 8) | (self.ram[index + 1] as u16);
    }

    fn op_3xkk(&mut self, x: usize, kk: u8) -> ProgramCounterAction {
        ProgramCounterAction::skip_if(self.v[x] == kk)
    }

    fn op_4xkk(&mut self, x: usize, kk: u8) -> ProgramCounterAction {
        ProgramCounterAction::skip_if(self.v[x] != kk)
    }

    fn op_5xy0(&mut self, x: usize, y: usize) -> ProgramCounterAction {
        ProgramCounterAction::skip_if(self.v[x] == self.v[y])
    }

    fn op_6xkk(&mut self, x: usize, kk: u8) -> ProgramCounterAction {
        self.v[x] = kk;
        ProgramCounterAction::Next
    }

    fn op_7xkk(&mut self, x: usize, kk: u8) -> ProgramCounterAction {
        self.v[x] += kk;
        ProgramCounterAction::Next
    }

    fn op_8xy0(&mut self, x: usize, y: usize) -> ProgramCounterAction {
        self.v[x] = self.v[y];
        ProgramCounterAction::Next
    }

    fn op_8xy1(&mut self, x: usize, y: usize) -> ProgramCounterAction {
        self.v[x] = self.v[x] | self.v[y];
        ProgramCounterAction::Next
    }

    fn op_8xy2(&mut self, x: usize, y: usize) -> ProgramCounterAction {
        self.v[x] = self.v[x] & self.v[y];
        ProgramCounterAction::Next
    }

    fn op_8xy3(&mut self, x: usize, y: usize) -> ProgramCounterAction {
        self.v[x] = self.v[x] ^ self.v[y];
        ProgramCounterAction::Next
    }

    fn op_8xy4(&mut self, x: usize, y: usize) -> ProgramCounterAction {
        let (result, overflow) = self.v[x].overflowing_add(self.v[y]);

        self.v[x] = result;
        self.v[0xf] = match overflow {
            true => 1,
            false => 0,
        };

        ProgramCounterAction::Next
    }

    fn op_8xy5(&mut self, x: usize, y: usize) -> ProgramCounterAction {
        let (result, overflow) = self.v[x].overflowing_sub(self.v[y]);

        self.v[x] = result;
        self.v[0xf] = match overflow {
            true => 1,
            false => 0,
        };

        ProgramCounterAction::Next
    }

    fn op_8xy6(&mut self, x: usize, y: usize) -> ProgramCounterAction {
        self.v[0xf] = self.v[x] & 0x1;
        self.v[x] = self.v[x] >> 1;

        ProgramCounterAction::Next 
    }

    fn op_8xy7(&mut self, x:usize, y:usize) -> ProgramCounterAction {
        let (result, overflow) = self.v[y].overflowing_sub(self.v[x]);

        self.v[x] = result;
        self.v[0xf] = match overflow {
            true => 1,
            false => 0,
        };

        ProgramCounterAction::Next
    }

    fn op_8xye(&mut self, x:usize, y:usize) -> ProgramCounterAction {
        let tmp = self.v[x] & 0b10000000;
        if tmp > 0 {
            self.v[0xf] = 1;
        }
        else {
            self.v[0xf] = 0;
        }

        self.v[x] = self.v[x] << 1;

        print!("v[x]={}, v[f]={}", self.v[x], self.v[0xf]);
        ProgramCounterAction::Next
    }

    // RET
    fn op_00ee(&mut self) -> ProgramCounterAction {
        let addr = self.stack[self.sp as usize];
        self.sp -= 1;
        ProgramCounterAction::Jump(addr)
    }

    // JMP insutrction.
    fn op_1nnn(&mut self, nnn: u16) -> ProgramCounterAction {
        ProgramCounterAction::Jump(nnn)
    }

    // CALL addr.
    // increment sp, then put pc on top of the stack
    fn op_2nnn(&mut self, nnn: u16) -> ProgramCounterAction {
        self.sp += 1;
        self.stack[self.sp as usize] = self.pc;

        ProgramCounterAction::Jump(nnn)
    }

    #[inline]
    // CLS: clear the screen.
    fn op_00e0(&mut self) -> ProgramCounterAction {
        for y in 0..CHIP8_HEIGHT {
            for x in 0..CHIP8_WIDTH {
                self.vram[y][x] = 0;
            }
        }

        ProgramCounterAction::Next
    }

    fn run(&mut self, opcode: u16) {
        let nibbles = (
            (opcode & 0xF000) >> 12,
            (opcode & 0x0F00) >> 8,
            (opcode & 0x00F0) >> 4,
            (opcode & 0x000F),
        );

        let nnn = opcode & 0x0FFF;
        let kk = (opcode & 0x00FF) as u8;
        let x = nibbles.1 as usize;
        let y = nibbles.2 as usize;
        let n = nibbles.3 as usize;

        let action = match nibbles {
            (0x0, 0x0, 0xe, 0x0) => self.op_00e0(),
            (0x0, 0x0, 0xe, 0xe) => self.op_00ee(),
            (0x1, _, _, _) => self.op_1nnn(nnn),
            (0x2, _, _, _) => self.op_2nnn(nnn),
            (0x3, _, _, _) => self.op_3xkk(x, kk),
            (0x4, _, _, _) => self.op_4xkk(x, kk),
            (0x5, _, _, 0x0) => self.op_5xy0(x, y),
            (0x6, _, _, _) => self.op_6xkk(x, kk),
            (0x7, _, _, _) => self.op_7xkk(x, kk),
            (0x8, _, _, 0x0) => self.op_8xy0(x, y),
            (0x8, _, _, 0x1) => self.op_8xy1(x, y),
            (0x8, _, _, 0x2) => self.op_8xy2(x, y),
            (0x8, _, _, 0x3) => self.op_8xy3(x, y),
            (0x8, _, _, 0x4) => self.op_8xy4(x, y),
            (0x8, _, _, 0x5) => self.op_8xy5(x, y),
            (0x8, _, _, 0x6) => self.op_8xy6(x, y),
            (0x8, _, _, 0x7) => self.op_8xy7(x, y),
            (0x8, _, _, 0xe) => self.op_8xye(x, y),
            _ => panic!("chip8.cpu: unimplemented instruction {:?}", nibbles),
        };

        match action {
            ProgramCounterAction::Next => self.pc += CHIP8_OPCODE_SIZE,
            ProgramCounterAction::Skip => self.pc += 2 * CHIP8_OPCODE_SIZE,
            ProgramCounterAction::Jump(addr) => self.pc = addr,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_initial_state() {
        let cpu = Cpu::new();

        for i in 0..CHIP8_FONT_SET_SIZE {
            assert_eq!(cpu.ram[i], FONT_SET[i]);
        }

        assert_eq!(cpu.pc, 0x200);
        assert_eq!(cpu.sp, 0x0);
    }

    #[test]
    fn test_cls_opcode() {
        let mut cpu = Cpu::new();
        cpu.run(0x00e0);

        // Check that PC is now pointing to next instruction.
        assert_eq!(cpu.pc, 0x200 + 2);

        // Check that the vram is actually cleared.
        for row in 0..CHIP8_HEIGHT {
            for col in 0..CHIP8_WIDTH {
                assert_eq!(cpu.vram[row][col], 0);
            }
        }
    }

    #[test]
    fn test_unimplemented_instruction_panics() {
        let result = std::panic::catch_unwind(|| {
            let mut cpu = Cpu::new();
            cpu.run(0x0022);
        });

        assert!(result.is_err());
    }

    #[test]
    fn test_read_opcode() {
        let mut cpu = Cpu::new();

        cpu.ram[0x200] = 0xB1;
        cpu.ram[0x201] = 0x5A;

        let opcode: u16 = cpu.read_opcode();
        assert_eq!(
            opcode, 0xB15A,
            "the opcode was correctly read using two bytes starting at PC."
        );
    }

    #[test]
    fn test_op_8xye() {
        let mut cpu = Cpu::new();
        cpu.v[1] = 0b10000001;
        cpu.run(0x812e);

        assert_eq!(cpu.v[0xf], 1, "Vf is set to carry");
        assert_eq!(cpu.v[1], 0b00000010, "Vx is set to Vx << 1");
    }
}
