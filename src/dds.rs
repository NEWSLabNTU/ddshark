use crate::qos::Qos;
use anyhow::{bail, ensure, Result};
use cyclors::{
    dds_builtintopic_endpoint_t, dds_create_listener, dds_create_participant, dds_create_reader,
    dds_create_statistics, dds_delete, dds_delete_listener, dds_delete_statistics, dds_entity_t,
    dds_get_instance_handle, dds_get_participant, dds_instance_handle_t,
    dds_instance_state_DDS_IST_ALIVE, dds_listener_t, dds_lset_data_available,
    dds_refresh_statistics, dds_return_loan, dds_return_t, dds_sample_info_t, dds_stat_kind,
    dds_stat_kind_DDS_STAT_KIND_LENGTHTIME, dds_stat_kind_DDS_STAT_KIND_UINT32,
    dds_stat_kind_DDS_STAT_KIND_UINT64, dds_statistics, dds_take, size_t,
    DDS_BUILTIN_TOPIC_DCPSPUBLICATION, DDS_BUILTIN_TOPIC_DCPSSUBSCRIPTION, DDS_RETCODE_ERROR,
    DDS_RETCODE_OK,
};
use itertools::izip;
use serde::{Deserialize, Serialize};
use std::{ffi::CStr, mem::MaybeUninit, os::raw::c_void, ptr, thread, time::Duration};
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

pub(crate) fn run_dds_discovery(domain_id: u32, tx: flume::Sender<DiscoveryEvent>) -> Result<()> {
    unsafe {
        // Create domain participant
        let domain_participant: dds_entity_t =
            dds_create_participant(domain_id, ptr::null(), ptr::null());

        // Create a listener for publishers
        let pub_context = Box::new(Context {
            pub_discovery: true,
            tx: tx.clone(),
        });
        let pub_context = Box::into_raw(pub_context);
        let pub_listener = dds_create_listener(pub_context as *mut c_void);
        dds_lset_data_available(pub_listener, Some(on_data));

        // Create a reader for publishers
        let pub_reader = dds_create_reader(
            domain_participant,
            DDS_BUILTIN_TOPIC_DCPSPUBLICATION,
            ptr::null(),
            pub_listener,
        );
        ensure!(
            pub_reader != DDS_RETCODE_ERROR,
            "dds_create_reader() failed"
        );

        // Create a statistics obj for publishers
        let pub_statistics = dds_create_statistics(pub_reader);
        ensure!(!pub_statistics.is_null(), "dds_create_statistics() failed");

        // Create a listener for subscribers
        let sub_context = Box::new(Context {
            pub_discovery: false,
            tx,
        });
        let sub_context = Box::into_raw(sub_context);
        let sub_listener = dds_create_listener(sub_context as *mut c_void);
        dds_lset_data_available(sub_listener, Some(on_data));

        // Create a reader for subscribers
        let sub_reader = dds_create_reader(
            domain_participant,
            DDS_BUILTIN_TOPIC_DCPSSUBSCRIPTION,
            ptr::null(),
            sub_listener,
        );
        ensure!(
            sub_reader != DDS_RETCODE_ERROR,
            "dds_create_reader() failed"
        );

        // Create a statistics obj for subscribers
        let sub_statistics = dds_create_statistics(sub_reader);
        ensure!(!sub_statistics.is_null(), "dds_create_statistics() failed");

        let check_rc = |rc: dds_return_t| {
            if rc != DDS_RETCODE_OK as i32 {
                error!("INTERNAL ERROR dds_delete() failed with return code {rc}");
            }
        };

        // Publish statistics periodically
        loop {
            let rc = dds_refresh_statistics(sub_statistics);
            ensure!(
                rc == DDS_RETCODE_OK as dds_return_t,
                "dds_refresh_statistics() failed"
            );
            let pub_statistics = pub_statistics.as_ref().unwrap();
            let kv_slice = pub_statistics.kv.as_slice(pub_statistics.count as usize);

            #[derive(Debug)]
            enum Value {
                U32(u32),
                U64(u64),
                LengthTime(u64),
            }

            kv_slice.iter().try_for_each(|kv| {
                let name = CStr::from_ptr(kv.name);
                let name = name.to_str().unwrap();

                let value = if kv.kind == dds_stat_kind_DDS_STAT_KIND_UINT32 {
                    Value::U32(kv.u.u32_)
                } else if kv.kind == dds_stat_kind_DDS_STAT_KIND_UINT64 {
                    Value::U64(kv.u.u64_)
                } else if kv.kind == dds_stat_kind_DDS_STAT_KIND_LENGTHTIME {
                    Value::LengthTime(kv.u.lengthtime)
                } else {
                    bail!("Invalid kv.kind value {}", kv.kind);
                };

                dbg!((name, value));
                anyhow::Ok(())
            })?;

            thread::sleep(Duration::from_secs(1));
        }

        // Finalize
        dds_delete_statistics(pub_statistics);
        dds_delete_statistics(sub_statistics);

        dds_delete_listener(pub_listener);
        dds_delete_listener(sub_listener);

        let rc = dds_delete(pub_reader);
        check_rc(rc);

        let rc = dds_delete(sub_reader);
        check_rc(rc);

        let rc = dds_delete(domain_participant);
        check_rc(rc);

        let _ = Box::from_raw(pub_context);
        let _ = Box::from_raw(sub_context);

        Ok(())
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
