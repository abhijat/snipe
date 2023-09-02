use std::collections::HashMap;

type BindingKey = String;

#[derive(Debug)]
enum BindingValue {
    Nothing,
    String(String),
    IndirectBinding {
        target: String,
        transformer: fn(&str) -> String,
    },
}

#[derive(Debug, Default)]
pub(crate) struct LazyBinding {
    bindings: HashMap<BindingKey, BindingValue>,
}

impl LazyBinding {
    pub fn add(&mut self, key: &str) {
        self.bindings.insert(key.to_owned(), BindingValue::Nothing);
    }

    pub fn add_transformed(&mut self, key: &str, target: &str, f: fn(&str) -> String) {
        assert!(
            self.bindings.contains_key(target),
            "{target} not in bindings"
        );

        self.bindings.insert(
            key.to_owned(),
            BindingValue::IndirectBinding {
                target: target.to_owned(),
                transformer: f,
            },
        );
    }

    pub fn populate(&mut self, key: &str, val: &str) {
        assert!(self.bindings.contains_key(key), "{key} is not in bindings");
        self.bindings
            .insert(key.to_owned(), BindingValue::String(val.to_owned()));
    }

    pub fn to_map(&self) -> HashMap<String, String> {
        let mut mapv: HashMap<String, String> = Default::default();
        for (k, v) in &self.bindings {
            match v {
                BindingValue::Nothing => todo!(),
                BindingValue::String(s) => mapv.insert(k.to_owned(), s.to_owned()),
                // first get bindings[target], then transform it
                BindingValue::IndirectBinding {
                    target,
                    transformer,
                } => {
                    assert!(
                        self.bindings.contains_key(target),
                        "{target} is not in bindings"
                    );
                    let key = self.bindings.get(target).unwrap();
                    let v = if let BindingValue::String(s) = key {
                        transformer(s)
                    } else {
                        panic!("unexpected binding for {:?}", key)
                    };
                    mapv.insert(k.to_owned(), v)
                }
            };
        }
        mapv
    }
}
