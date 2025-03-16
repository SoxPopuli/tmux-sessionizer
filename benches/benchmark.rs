use criterion::{Criterion, black_box, criterion_group, criterion_main};

trait SearchPathHelper {
    fn simple(path: impl Into<String>) -> SearchPath {
        SearchPath::Simple(path.into()).expand().unwrap()
    }

    fn complex(path: impl Into<String>, depth: Option<u8>) -> SearchPath {
        SearchPath::Complex {
            path: path.into(),
            depth,
            show_hidden: Some(true),
        }
        .expand()
        .unwrap()
    }
}
impl SearchPathHelper for SearchPath {}

use tmux_sessionizer::config::{Config, SearchPath, Settings};

fn find_all_dirs(c: &mut Criterion) {
    let config = Config {
        paths: vec![
            SearchPath::simple("~/Code"),
            SearchPath::simple("~/Documents/Work"),
            SearchPath::complex("~/Documents", Some(1)),
            SearchPath::complex("~/.config", Some(1)),
            SearchPath::complex("~/vaults", Some(0)),
        ],
        settings: Settings {
            default_depth: 8,
            picker: None,
        },
    };

    c.bench_function("find_dirs", |b| {
        b.iter(|| black_box(config.find_dirs().unwrap()));
    });
}

criterion_group!(benches, find_all_dirs);
criterion_main!(benches);
