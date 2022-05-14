use pyo3::prelude::*;

#[pyclass]
struct SpacyPlugin {

}

#[pymethods]
impl SpacyPlugin {
    #[new]
    fn new() -> Self {
        Self {  }
    }
}

#[pymodule]
fn spacy_plugin(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<SpacyPlugin>()?;
    Ok(())
}
