use arm64::exceptions::*;
pub struct Excps;

impl Exceptions<ELx_SP_EL0> for Excps {}

impl Exceptions<ELx_SP_ELx> for Excps {
    fn sync_excp(_frame: &mut ExceptionFrame) {
        loop {}
    }

    fn serror(_frame: &mut ExceptionFrame) {
        loop {}
    }
}

impl Exceptions<ELy_AARCH64> for Excps {}

impl Exceptions<ELy_AARCH32> for Excps {}
