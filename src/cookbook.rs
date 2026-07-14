//! Deterministic cookbook builders for stream-host recipes.

use sim_kernel::{Expr, NumberLiteral, Symbol};

use crate::{FakeBackend, HostBackend};

/// Build the modeled fake backend descriptor used by the cookbook recipe.
pub fn fake_backend_demo() -> Expr {
    let backend = FakeBackend::new();
    let inventory = backend.enumerate().expect("fake backend enumerates");
    let opened = backend
        .open(FakeBackend::data_request(8).expect("valid fake data request"))
        .expect("fake backend opens data stream");
    let config = opened.config();

    Expr::Map(vec![
        (field("kind"), sym("stream-host", "fake-backend")),
        (field("backend"), Expr::Symbol(config.backend().clone())),
        (field("device"), Expr::Symbol(config.device().clone())),
        (field("media"), Expr::Symbol(config.media().symbol())),
        (field("clock"), Expr::Symbol(config.clock().clock().clone())),
        (field("devices"), number(inventory.devices().len())),
        (field("ports"), number(inventory.ports().len())),
        (field("callback-queue"), sym("stream-host", "bounded")),
        (field("hardware-required"), Expr::Bool(false)),
    ])
}

fn field(name: &str) -> Expr {
    Expr::Symbol(Symbol::qualified("stream-host", name))
}

fn sym(namespace: &str, name: &str) -> Expr {
    Expr::Symbol(Symbol::qualified(namespace, name))
}

fn number(value: impl ToString) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "i64"),
        canonical: value.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_lib_stream_core::StreamMedia;

    #[test]
    fn fake_backend_demo_opens_modeled_data_stream() {
        let Expr::Map(entries) = fake_backend_demo() else {
            panic!("fake backend demo is a map")
        };
        assert!(entries.iter().any(|(_, value)| {
            matches!(value, Expr::Symbol(symbol) if symbol.as_qualified_str() == "stream/media/data")
        }));
        assert!(entries.iter().any(|(_, value)| *value == Expr::Bool(false)));
        let backend = FakeBackend::new();
        let opened = backend
            .open(FakeBackend::data_request(4).expect("request"))
            .expect("open");
        assert_eq!(opened.config().media(), StreamMedia::Data);
    }
}
