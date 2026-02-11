use crate::core::aiserver::v1::ExplicitContext;
use byte_str::ByteStr;
use manually_init::ManuallyInit;
use rep_move::RepMove;

static CONTEXT_FILL_MODE: ManuallyInit<ContextFillMode> = ManuallyInit::new();

#[inline]
pub fn init() {
    let context_fill_mode = crate::common::utils::parse_from_env("CONTEXT_FILL_MODE", 1u8);
    CONTEXT_FILL_MODE.init(ContextFillMode::from_bits(context_fill_mode));
}

#[derive(Clone, Copy)]
pub struct ContextFillMode {
    context: bool,
    repo_context: bool,
    mode_specific_context: bool,
}

impl ContextFillMode {
    #[inline]
    const fn from_bits(value: u8) -> Self {
        Self {
            context: value & 0b001 != 0,
            repo_context: value & 0b010 != 0,
            mode_specific_context: value & 0b100 != 0,
        }
    }
}

/// Create ExplicitContext based on global configuration and instruction
#[inline]
pub fn create_explicit_context(instructions: ByteStr) -> Option<ExplicitContext> {
    if instructions.trim().is_empty() {
        return None;
    }

    let mode = *CONTEXT_FILL_MODE;

    // bool as usize: true = 1, false = 0
    let count =
        mode.context as usize + mode.repo_context as usize + mode.mode_specific_context as usize;

    match count {
        0 => None,
        1 => {
            // Single copy case: zero clones, direct move
            let mut ctx = ExplicitContext::default();
            if mode.context {
                ctx.context = instructions;
            } else if mode.repo_context {
                ctx.repo_context = Some(instructions);
            } else {
                ctx.mode_specific_context = Some(instructions);
            }
            Some(ctx)
        }
        _ => {
            // Multiple copy case: use RepMove to optimize last move
            let mut iter = RepMove::new(instructions, Clone::clone, count);

            Some(ExplicitContext {
                context: if mode.context { iter.next().unwrap() } else { ByteStr::new() },
                repo_context: if mode.repo_context { iter.next() } else { None },
                mode_specific_context: if mode.mode_specific_context { iter.next() } else { None },
            })
        }
    }
}
