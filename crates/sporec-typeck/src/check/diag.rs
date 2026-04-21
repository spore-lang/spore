use super::*;

impl Checker {
    pub(super) fn err(&mut self, code: ErrorCode, message: String) {
        self.errors.push(TypeError::new(code, message));
    }

    pub(super) fn err_at(&mut self, code: ErrorCode, message: String, span: Span) {
        self.errors.push(TypeError::with_span(code, message, span));
    }
}
