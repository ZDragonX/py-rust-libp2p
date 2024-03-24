pub mod p2p;
pub mod file_tools;
pub mod ecdsa_tools;

use p2p::P2PNetwork;

use lazy_static::lazy_static;
use pyo3::prelude::*;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;
use pyo3::types::PyBytes;
use pyo3::types::PyList;


lazy_static! {
    static ref GLOBAL_P2P_NETWORK: Arc<AsyncMutex<Option<P2PNetwork>>> = Arc::new(AsyncMutex::new(None));
}

#[pyfunction]
fn init_global_p2p_network(py: Python<'_>, bootnodes: Vec<String>, port: u64, key_path: String) -> PyResult<&PyAny> {
    let tp = GLOBAL_P2P_NETWORK.clone();
    pyo3_asyncio::tokio::future_into_py(py, async move {
        let p2p_net = P2PNetwork::new(bootnodes, port, key_path)
            .await.expect("start p2p server error");
        {
            let mut network = tp.lock().await;
            *network = Some(p2p_net);
        }
        Ok(Python::with_gil(|py| py.None()))
    })
}

#[pyfunction]
fn get_global_connected_peers(py: Python<'_>) -> PyResult<&PyAny> {
    let tp = GLOBAL_P2P_NETWORK.clone();
    pyo3_asyncio::tokio::future_into_py(py, async move {
        let network = tp.lock().await;

        if let Some(network) = &*network {
            let peers = network.get_connected_peers().await.expect("Failed to get connected peers");
            Python::with_gil(|py| {
                // 将Rust Vec<String>转换为Python列表
                Ok(peers.into_iter().map(|peer| peer.to_string()).collect::<Vec<String>>().into_py(py))
            })
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("P2PNetwork is not initialized"))
        }
    })
}

#[pyfunction]
fn connect_to_peer(py: Python<'_>, addr: String) -> PyResult<&PyAny> {
    let tp = GLOBAL_P2P_NETWORK.clone();
    pyo3_asyncio::tokio::future_into_py(py, async move {
        let network = tp.lock().await;

        if let Some(network) = &*network {
            network.connect_to_peer(&addr).await.map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("Failed to connect to peer: {}", e))
            })?;
            Ok(Python::with_gil(|py| py.None()))
        }else {
            Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("P2PNetwork is not initialized"))
        }
    })
}

#[pyfunction]
fn publish_message(py: Python<'_>, message: Vec<u8>) -> PyResult<&PyAny> {
    let tp = GLOBAL_P2P_NETWORK.clone();
    pyo3_asyncio::tokio::future_into_py(py, async move {
        let network = tp.lock().await;

        if let Some(network) = &*network {
            network.publish_message(message).await.map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("Failed to publish message: {}", e))
            })?;
            Ok(Python::with_gil(|py| py.None()))
        }else {
            Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("P2PNetwork is not initialized"))
        }
    })
}

#[pyfunction]
fn subscribe_to_messages(py: Python, callback: PyObject) -> PyResult<&PyAny> {
    let tp = GLOBAL_P2P_NETWORK.clone();
    pyo3_asyncio::tokio::future_into_py(py, async move {
        let mut receiver_result = {
            let network = tp.lock().await;
            network.as_ref()
                .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("P2PNetwork is not initialized"))?
                .subscribe_to_messages()
        };

        while let Ok(message_event) = receiver_result.recv().await {
            // println!("Received message from {:?}: {:?}", message_event.source, message_event.content);
            Python::with_gil(|py| {
                // 将消息转换为Python的bytes对象
                let py_message = PyBytes::new(py, &message_event.content);
                // 调用Python的回调函数
                callback.call1(py, (py_message, )).unwrap();
            });
        }
        Ok(Python::with_gil(|py| py.None()))
    })
}

#[pyfunction]
fn get_host_addrs(py: Python<'_>) -> PyResult<&PyAny> {
    let tp = GLOBAL_P2P_NETWORK.clone();
    pyo3_asyncio::tokio::future_into_py(py, async move {
        let network = tp.lock().await;
        if let Some(network) = &*network {
            let addrs = network.full_addrs.read().await;
            Python::with_gil(|py| {
                // 将Rust Vec<String>转换为Python列表
                Ok(addrs.clone().into_iter().map(|peer| peer.to_string()).collect::<Vec<String>>().into_py(py))
            })
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("P2PNetwork is not initialized"))
        }
    })
}

#[pyfunction]
fn get_peer_id(py: Python<'_>) -> PyResult<&PyAny> {
    let tp = GLOBAL_P2P_NETWORK.clone();
    pyo3_asyncio::tokio::future_into_py(py, async move {
        let network = tp.lock().await;
        if let Some(network) = &*network {
            let peer_id = network.peer_id.to_string();
            Python::with_gil(|py| {
                // 将Rust Vec<String>转换为Python列表
                Ok(peer_id)
            })
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("P2PNetwork is not initialized"))
        }
    })
}


#[pyfunction]
fn generate_ed25519_keypair(path: String) -> PyResult<String> {
    let keypair = file_tools::generate_ed25519_keypair();
    file_tools::save_keypair_to_file(&keypair, path.clone())?;
    Ok(path)
}

#[pyfunction]
fn sign_data(file_path_str: String, data: Vec<u8>) -> PyResult<String> {
    let keypair = file_tools::sign_data(file_path_str, data)?;
    Ok(keypair)
}

#[pyfunction]
fn get_serialize_public_key(path: String) -> PyResult<String> {
    let pub_key = file_tools::get_serialize_public_key(path)?;
    Ok(pub_key)
}

#[pymodule]
fn p2p_helper(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(init_global_p2p_network, m)?)?;
    m.add_function(wrap_pyfunction!(get_global_connected_peers, m)?)?;
    m.add_function(wrap_pyfunction!(connect_to_peer, m)?)?;
    m.add_function(wrap_pyfunction!(publish_message, m)?)?;
    m.add_function(wrap_pyfunction!(subscribe_to_messages, m)?)?;
    m.add_function(wrap_pyfunction!(generate_ed25519_keypair, m)?)?;
    m.add_function(wrap_pyfunction!(get_host_addrs, m)?)?;
    m.add_function(wrap_pyfunction!(get_peer_id, m)?)?;
    m.add_function(wrap_pyfunction!(sign_data, m)?)?;
    m.add_function(wrap_pyfunction!(get_serialize_public_key, m)?)?;
    Ok(())
}