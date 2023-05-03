use crate::qos::Qos;
use anyhow::{ensure, Result};
use cyclors::{
    dds_builtintopic_endpoint_t, dds_create_listener, dds_create_participant, dds_create_reader,
    dds_delete, dds_delete_listener, dds_entity_t, dds_get_instance_handle, dds_get_participant,
    dds_instance_handle_t, dds_instance_state_DDS_IST_ALIVE, dds_listener_t,
    dds_lset_data_available, dds_return_loan, dds_return_t, dds_sample_info_t, dds_take, size_t,
    DDS_BUILTIN_TOPIC_DCPSPUBLICATION, DDS_BUILTIN_TOPIC_DCPSSUBSCRIPTION, DDS_RETCODE_ERROR,
    DDS_RETCODE_OK,
};
use itertools::izip;
use serde::{Deserialize, Serialize};
use std::{ffi::CStr, mem::MaybeUninit, os::raw::c_void, ptr};
use tracing::{debug, error, warn};

const MAX_SAMPLES: usize = 32;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct DdsEntity {
    pub(crate) key: String,
    pub(crate) participant_key: String,
    pub(crate) topic_name: String,
    pub(crate) type_name: String,
    pub(crate) keyless: bool,
    pub(crate) qos: Qos,
    // pub(crate) routes: HashMap<String, RouteStatus>, // map of routes statuses indexed by partition ("*" only if no partition)
}

pub(crate) struct DdsDiscoveryHandle {
    domain_participant: dds_entity_t,
    pub_reader: dds_entity_t,
    pub_listener: *mut dds_listener_t,
    sub_reader: dds_entity_t,
    sub_listener: *mut dds_listener_t,
    pub_context: *mut Context,
    sub_context: *mut Context,
}

impl DdsDiscoveryHandle {
    pub fn start(domain_id: u32, tx: flume::Sender<DiscoveryEvent>) -> Result<Self> {
        unsafe {
            let domain_participant: dds_entity_t =
                dds_create_participant(domain_id, ptr::null(), ptr::null());

            let (pub_reader, pub_listener, pub_context) = {
                let ptx = Box::new(Context {
                    pub_discovery: true,
                    tx: tx.clone(),
                });
                let ptx = Box::into_raw(ptx);
                let sub_listener = dds_create_listener(ptx as *mut c_void);

                dds_lset_data_available(sub_listener, Some(on_data));
                let pr = dds_create_reader(
                    domain_participant,
                    DDS_BUILTIN_TOPIC_DCPSPUBLICATION,
                    ptr::null(),
                    sub_listener,
                );
                ensure!(pr != DDS_RETCODE_ERROR, "dds_create_reader() failed");

                (pr, sub_listener, ptx)
            };

            let (sub_reader, sub_listener, sub_context) = {
                let stx = Box::new(Context {
                    pub_discovery: false,
                    tx,
                });
                let stx = Box::into_raw(stx);
                let sub_listener = dds_create_listener(stx as *mut c_void);

                dds_lset_data_available(sub_listener, Some(on_data));
                let sr = dds_create_reader(
                    domain_participant,
                    DDS_BUILTIN_TOPIC_DCPSSUBSCRIPTION,
                    ptr::null(),
                    sub_listener,
                );
                ensure!(sr != DDS_RETCODE_ERROR, "dds_create_reader() failed");

                (sr, sub_listener, stx)
            };

            Ok(Self {
                pub_reader,
                sub_reader,
                domain_participant,
                pub_listener,
                sub_listener,
                pub_context,
                sub_context,
            })
        }
    }

    pub fn stop(self) {}
}

impl Drop for DdsDiscoveryHandle {
    fn drop(&mut self) {
        let check_rc = |rc: dds_return_t| {
            if rc != DDS_RETCODE_OK as i32 {
                error!("INTERNAL ERROR dds_delete() failed with return code {rc}");
            }
        };

        unsafe {
            dds_delete_listener(self.pub_listener);
            dds_delete_listener(self.sub_listener);

            let rc = dds_delete(self.pub_reader);
            check_rc(rc);

            let rc = dds_delete(self.sub_reader);
            check_rc(rc);

            let rc = dds_delete(self.domain_participant);
            check_rc(rc);

            let _ = Box::from_raw(self.pub_context);
            let _ = Box::from_raw(self.sub_context);
        }
    }
}

#[derive(Debug)]
pub(crate) enum DiscoveryEvent {
    DiscoveredPublication { entity: DdsEntity },
    UndiscoveredPublication { key: String },
    DiscoveredSubscription { entity: DdsEntity },
    UndiscoveredSubscription { key: String },
}

struct Context {
    pub pub_discovery: bool,
    pub tx: flume::Sender<DiscoveryEvent>,
}

fn send_discovery_event(sender: &flume::Sender<DiscoveryEvent>, event: DiscoveryEvent) {
    if let Err(err) = sender.try_send(event) {
        error!(
            "INTERNAL ERROR sending DiscoveryEvent to internal channel: {:?}",
            err
        );
    }
}

unsafe extern "C" fn on_data(dr: dds_entity_t, arg: *mut c_void) {
    // Load arguments
    let btx = Box::from_raw(arg as *mut Context);
    let Context {
        pub_discovery,
        ref tx,
    } = *btx;

    // Get self instance handle
    let dpih = {
        let dp = dds_get_participant(dr);
        let mut dpih: dds_instance_handle_t = 0;
        let _ = dds_get_instance_handle(dp, &mut dpih);
        dpih
    };

    // Run dds_take() to load samples
    let (n_samples, si, mut samples) = {
        let mut si: MaybeUninit<[dds_sample_info_t; MAX_SAMPLES]> = MaybeUninit::uninit();
        let mut samples: [*mut c_void; MAX_SAMPLES] = [ptr::null_mut(); MAX_SAMPLES];

        let n_samples = dds_take(
            dr,
            samples.as_mut_ptr() as *mut *mut c_void,
            si.as_mut_ptr() as *mut dds_sample_info_t,
            MAX_SAMPLES as size_t,
            MAX_SAMPLES as u32,
        ) as usize;

        let si = si.assume_init();
        (n_samples, si, samples)
    };

    for (info, sample) in izip!(&si[0..n_samples], samples[0..n_samples].iter().copied()) {
        let sample = (sample as *mut dds_builtintopic_endpoint_t)
            .as_ref()
            .unwrap();

        // Ignore discovery of entities created by our own participant
        if sample.participant_instance_handle == dpih {
            continue;
        }

        let is_alive = info.instance_state == dds_instance_state_DDS_IST_ALIVE;
        let key = hex::encode(sample.key.v);

        if is_alive {
            // Get topic name
            let topic_name = match CStr::from_ptr(sample.topic_name).to_str() {
                Ok(s) => s,
                Err(e) => {
                    warn!("Discovery of an invalid topic name: {}", e);
                    continue;
                }
            };
            if topic_name.starts_with("DCPS") {
                debug!(
                    "Ignoring discovery of {} ({} is a builtin topic)",
                    key, topic_name
                );
                continue;
            }

            // Get type name
            let type_name = match CStr::from_ptr(sample.type_name).to_str() {
                Ok(s) => s,
                Err(e) => {
                    warn!("Discovery of an invalid topic type: {}", e);
                    continue;
                }
            };

            // Get keys
            let participant_key = hex::encode(sample.participant_key.v);
            let keyless = sample.key.v[15] == 3 || sample.key.v[15] == 4;

            debug!(
                "Discovered DDS {} {} from Participant {} on {} with type {} (keyless: {})",
                if pub_discovery {
                    "publication"
                } else {
                    "subscription"
                },
                key,
                participant_key,
                topic_name,
                type_name,
                keyless
            );

            let qos = if pub_discovery {
                Qos::from_writer_qos_native(sample.qos)
            } else {
                Qos::from_reader_qos_native(sample.qos)
            };

            // send a DiscoveryEvent
            let entity = DdsEntity {
                key: key.clone(),
                participant_key: participant_key.clone(),
                topic_name: String::from(topic_name),
                type_name: String::from(type_name),
                keyless,
                qos,
                // routes: HashMap::<String, RouteStatus>::new(),
            };

            if pub_discovery {
                send_discovery_event(tx, DiscoveryEvent::DiscoveredPublication { entity });
            } else {
                send_discovery_event(tx, DiscoveryEvent::DiscoveredSubscription { entity });
            }
        } else if pub_discovery {
            send_discovery_event(tx, DiscoveryEvent::UndiscoveredPublication { key });
        } else {
            send_discovery_event(tx, DiscoveryEvent::UndiscoveredSubscription { key });
        }
    }

    dds_return_loan(
        dr,
        samples.as_mut_ptr() as *mut *mut c_void,
        MAX_SAMPLES as i32,
    );
    Box::into_raw(btx);
}
