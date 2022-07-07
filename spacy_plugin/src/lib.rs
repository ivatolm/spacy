use std::net::TcpStream;
use pyo3::prelude::*;

#[pyclass(subclass)]
struct SpacyPlugin {
    pub stream: TcpStream
}

#[pymethods]
impl SpacyPlugin {
    #[new]
    fn new() -> Self {
        let stream = TcpStream::connect(("127.0.0.1", 32002)).unwrap();

        Self { stream }
    }
}

#[pymodule]
fn spacy_plugin(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<SpacyPlugin>()?;
    Ok(())
}
