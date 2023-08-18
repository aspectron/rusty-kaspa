extern crate kaspa_consensus;
extern crate kaspa_core;
extern crate kaspa_hashes;

use kaspa_addressmanager::AddressManager;
use kaspa_consensus::consensus::factory::Factory as ConsensusFactory;
use kaspa_consensus::pipeline::monitor::ConsensusMonitor;
use kaspa_consensus::pipeline::ProcessingCounters;
use kaspa_consensus_notify::root::ConsensusNotificationRoot;
use kaspa_consensus_notify::service::NotifyService;
use kaspa_consensusmanager::ConsensusManager;
use kaspa_core::task::tick::TickService;
use kaspa_core::{core::Core, task::runtime::AsyncRuntime};
use kaspa_index_processor::service::IndexService;
use kaspa_mining::manager::{MiningManager, MiningManagerProxy};
use kaspa_p2p_flows::flow_context::FlowContext;
use kaspa_rpc_service::service::RpcCoreService;
use kaspa_utils::networking::ContextualNetAddress;
use kaspa_utxoindex::api::UtxoIndexProxy;

use std::fs;
use std::sync::Arc;
use std::time::Duration;

use crate::args::Args;
use crate::context::Context;
use crate::runtime::Runtime;
use crate::utils::*;

use async_channel::unbounded;
use kaspa_core::{info, trace};
use kaspa_grpc_server::service::GrpcService;
use kaspa_p2p_flows::service::P2pService;
use kaspa_perf_monitor::builder::Builder as PerfMonitorBuilder;
use kaspa_utxoindex::UtxoIndex;
use kaspa_wrpc_server::service::{Options as WrpcServerOptions, ServerCounters as WrpcServerCounters, WrpcEncoding, WrpcService};

#[cfg(feature = "heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[derive(Clone)]
pub struct Kaspad {
    core: Arc<Core>,
}

impl Kaspad {
    pub fn core(&self) -> &Arc<Core> {
        &self.core
    }

    pub fn run(&self) {
        self.core.run();
    }

    pub fn new(runtime: &Runtime, context: Context, args: Args) -> Kaspad {
        let Runtime { app_dir: rt_app_dir, db_dir: rt_db_dir, log_dir, network: rt_network } = runtime;
        let Context { app_dir: ctx_app_dir, db_dir: ctx_db_dir, config, network: ctx_network } = context;

        let app_dir = rt_app_dir.clone().or(ctx_app_dir).expect("app_dir is required as a part of the daemon Runtime or Context");
        let db_dir = rt_db_dir.clone().or(ctx_db_dir).expect("db_dir is required as a part of the daemon Runtime or Context");
        let network = rt_network.clone().or(ctx_network).expect("network is required as a part of the daemon Runtime or Context");

        // ---

        assert!(!db_dir.to_str().unwrap().is_empty());
        info!("Application directory: {}", app_dir.display());
        info!("Data directory: {}", db_dir.display());
        match log_dir {
            Some(s) => {
                info!("Logs directory: {}", s);
            }
            None => {
                info!("Logs to console only");
            }
        }

        let consensus_db_dir = db_dir.join(CONSENSUS_DB);
        let utxoindex_db_dir = db_dir.join(UTXOINDEX_DB);
        let meta_db_dir = db_dir.join(META_DB);

        if args.reset_db && db_dir.exists() {
            let msg = "Reset DB was requested -- this means the current databases will be fully deleted, 
    do you confirm? (answer y/n or pass --yes to the Kaspad command line to confirm all interactive questions)";
            get_user_approval_or_exit(msg, args.yes);
            info!("Deleting databases");
            fs::remove_dir_all(db_dir.clone()).unwrap();
        }

        fs::create_dir_all(consensus_db_dir.as_path()).unwrap();
        fs::create_dir_all(meta_db_dir.as_path()).unwrap();
        if args.utxoindex {
            info!("Utxoindex Data directory {}", utxoindex_db_dir.display());
            fs::create_dir_all(utxoindex_db_dir.as_path()).unwrap();
        }

        // DB used for addresses store and for multi-consensus management
        let mut meta_db = kaspa_database::prelude::ConnBuilder::default().with_db_path(meta_db_dir.clone()).build();

        // TEMP: upgrade from Alpha version or any version before this one
        if meta_db.get_pinned(b"multi-consensus-metadata-key").is_ok_and(|r| r.is_some()) {
            let msg = "Node database is from an older Kaspad version and needs to be fully deleted, do you confirm the delete? (y/n)";
            get_user_approval_or_exit(msg, args.yes);

            info!("Deleting databases from previous Kaspad version");

            // Drop so that deletion works
            drop(meta_db);

            // Delete
            fs::remove_dir_all(db_dir).unwrap();

            // Recreate the empty folders
            fs::create_dir_all(consensus_db_dir.as_path()).unwrap();
            fs::create_dir_all(meta_db_dir.as_path()).unwrap();
            fs::create_dir_all(utxoindex_db_dir.as_path()).unwrap();

            // Reopen the DB
            meta_db = kaspa_database::prelude::ConnBuilder::default().with_db_path(meta_db_dir).build();
        }

        let connect_peers = args.connect_peers.iter().map(|x| x.normalize(config.default_p2p_port())).collect::<Vec<_>>();
        let add_peers = args.add_peers.iter().map(|x| x.normalize(config.default_p2p_port())).collect();
        let p2p_server_addr = args.listen.unwrap_or(ContextualNetAddress::unspecified()).normalize(config.default_p2p_port());
        // connect_peers means no DNS seeding and no outbound peers
        let outbound_target = if connect_peers.is_empty() { args.outbound_target } else { 0 };
        let dns_seeders = if connect_peers.is_empty() { config.dns_seeders } else { &[] };

        let grpc_server_addr = args.rpclisten.unwrap_or(ContextualNetAddress::unspecified()).normalize(config.default_rpc_port());

        let core = Arc::new(Core::new());

        // ---

        let tick_service = Arc::new(TickService::new());
        let (notification_send, notification_recv) = unbounded();
        let notification_root = Arc::new(ConsensusNotificationRoot::new(notification_send));
        let processing_counters = Arc::new(ProcessingCounters::default());
        let wrpc_borsh_counters = Arc::new(WrpcServerCounters::default());
        let wrpc_json_counters = Arc::new(WrpcServerCounters::default());

        // Use `num_cpus` background threads for the consensus database as recommended by rocksdb
        let consensus_db_parallelism = num_cpus::get();
        let consensus_factory = Arc::new(ConsensusFactory::new(
            meta_db.clone(),
            &config,
            consensus_db_dir,
            consensus_db_parallelism,
            notification_root.clone(),
            processing_counters.clone(),
        ));

        let consensus_manager = Arc::new(ConsensusManager::new(consensus_factory));
        let consensus_monitor = Arc::new(ConsensusMonitor::new(processing_counters.clone(), tick_service.clone()));

        let perf_monitor_builder = PerfMonitorBuilder::new()
            .with_fetch_interval(Duration::from_secs(args.perf_metrics_interval_sec))
            .with_tick_service(tick_service.clone());
        let perf_monitor = if args.perf_metrics {
            let cb = move |counters| {
                trace!("[{}] metrics: {:?}", kaspa_perf_monitor::SERVICE_NAME, counters);
                #[cfg(feature = "heap")]
                trace!("heap stats: {:?}", dhat::HeapStats::get());
            };
            Arc::new(perf_monitor_builder.with_fetch_cb(cb).build())
        } else {
            Arc::new(perf_monitor_builder.build())
        };

        let notify_service = Arc::new(NotifyService::new(notification_root.clone(), notification_recv));
        let index_service: Option<Arc<IndexService>> = if args.utxoindex {
            // Use only a single thread for none-consensus databases
            let utxoindex_db = kaspa_database::prelude::ConnBuilder::default().with_db_path(utxoindex_db_dir).build();
            let utxoindex = UtxoIndexProxy::new(UtxoIndex::new(consensus_manager.clone(), utxoindex_db).unwrap());
            let index_service = Arc::new(IndexService::new(&notify_service.notifier(), Some(utxoindex)));
            Some(index_service)
        } else {
            None
        };

        let address_manager = AddressManager::new(config.clone(), meta_db);
        let mining_manager =
            MiningManagerProxy::new(Arc::new(MiningManager::new(config.target_time_per_block, false, config.max_block_mass, None)));

        let flow_context = Arc::new(FlowContext::new(
            consensus_manager.clone(),
            address_manager,
            config.clone(),
            mining_manager.clone(),
            tick_service.clone(),
            notification_root,
        ));
        let p2p_service = Arc::new(P2pService::new(
            flow_context.clone(),
            connect_peers,
            add_peers,
            p2p_server_addr,
            outbound_target,
            args.inbound_limit,
            dns_seeders,
            config.default_p2p_port(),
        ));

        let rpc_core_service = Arc::new(RpcCoreService::new(
            consensus_manager.clone(),
            notify_service.notifier(),
            index_service.as_ref().map(|x| x.notifier()),
            mining_manager,
            flow_context,
            index_service.as_ref().map(|x| x.utxoindex().unwrap()),
            config,
            core.clone(),
            processing_counters,
            wrpc_borsh_counters.clone(),
            wrpc_json_counters.clone(),
            perf_monitor.clone(),
        ));
        let grpc_service = Arc::new(GrpcService::new(grpc_server_addr, rpc_core_service.clone(), args.rpc_max_clients));

        // Create an async runtime and register the top-level async services
        let async_runtime = Arc::new(AsyncRuntime::new(args.async_threads));
        async_runtime.register(tick_service);
        async_runtime.register(notify_service);
        if let Some(index_service) = index_service {
            async_runtime.register(index_service)
        };
        async_runtime.register(rpc_core_service.clone());
        async_runtime.register(grpc_service);
        async_runtime.register(p2p_service);
        async_runtime.register(consensus_monitor);
        async_runtime.register(perf_monitor);
        let wrpc_service_tasks: usize = 2; // num_cpus::get() / 2;
                                           // Register wRPC servers based on command line arguments
        [
            (args.rpclisten_borsh, WrpcEncoding::Borsh, wrpc_borsh_counters),
            (args.rpclisten_json, WrpcEncoding::SerdeJson, wrpc_json_counters),
        ]
        .into_iter()
        .filter_map(|(listen_address, encoding, wrpc_server_counters)| {
            listen_address.map(|listen_address| {
                Arc::new(WrpcService::new(
                    wrpc_service_tasks,
                    Some(rpc_core_service.clone()),
                    &encoding,
                    wrpc_server_counters,
                    WrpcServerOptions {
                        listen_address: listen_address.to_address(&network.network_type, &encoding).to_string(), // TODO: use a normalized ContextualNetAddress instead of a String
                        verbose: args.wrpc_verbose,
                        ..WrpcServerOptions::default()
                    },
                ))
            })
        })
        .for_each(|server| async_runtime.register(server));

        // Consensus must start first in order to init genesis in stores
        core.bind(consensus_manager);
        core.bind(async_runtime);

        Kaspad { core }
    }
}
