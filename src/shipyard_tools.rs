//====================================================================

use ahash::{HashMap, HashMapExt};

//====================================================================

pub type Res<'a, T> = shipyard::UniqueView<'a, T>;
pub type ResMut<'a, T> = shipyard::UniqueViewMut<'a, T>;

//====================================================================

#[allow(dead_code)]
pub trait WorldTools {
    fn and_run<B, S: shipyard::System<(), B>>(&self, system: S) -> &Self;
    fn and_run_with_data<Data, B, S: shipyard::System<(Data,), B>>(
        &self,
        system: S,
        data: Data,
    ) -> &Self;
}

impl WorldTools for shipyard::World {
    #[inline]
    fn and_run<B, S: shipyard::System<(), B>>(&self, system: S) -> &Self {
        self.run(system);
        self
    }

    #[inline]
    fn and_run_with_data<Data, B, S: shipyard::System<(Data,), B>>(
        &self,
        system: S,
        data: Data,
    ) -> &Self {
        self.run_with_data(system, data);
        self
    }
}

#[allow(dead_code)]
pub trait UniqueTools {
    fn insert<U: shipyard::Unique + Send + Sync>(&self, unique: U) -> &Self;
    fn replace<U: shipyard::Unique + Send + Sync>(&self, unique: U);
}

impl UniqueTools for shipyard::World {
    #[inline]
    fn insert<U: shipyard::Unique + Send + Sync>(&self, unique: U) -> &Self {
        self.add_unique(unique);
        self
    }

    fn replace<U: shipyard::Unique + Send + Sync>(&self, unique: U) {
        self.remove_unique::<U>().ok();
        self.add_unique(unique);
    }
}

impl UniqueTools for shipyard::AllStoragesView<'_> {
    #[inline]
    fn insert<U: shipyard::Unique + Send + Sync>(&self, unique: U) -> &Self {
        self.add_unique(unique);
        self
    }

    fn replace<U: shipyard::Unique + Send + Sync>(&self, unique: U) {
        self.remove_unique::<U>().ok();
        self.add_unique(unique);
    }
}

//====================================================================

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug, shipyard::Label, enum_iterator::Sequence)]
pub enum Stages {
    PreSetup,
    Setup,
    PostSetup,

    First,

    PreUpdate,
    Update,
    PostUpdate,

    PreRender,
    Render,
    PostRender,

    Last,

    Resize,
}

pub struct WorkloadBuilder<'a> {
    world: &'a shipyard::World,
    workloads: HashMap<Stages, shipyard::Workload>,
}

impl<'a> WorkloadBuilder<'a> {
    pub fn new(world: &'a shipyard::World) -> Self {
        Self {
            world,
            workloads: HashMap::new(),
        }
    }

    pub fn add_workload(&mut self, stage: Stages, workload: shipyard::Workload) -> &mut Self {
        log::trace!("Adding workload for stage '{:?}'", stage);
        let old_workload = self
            .workloads
            .remove(&stage)
            .unwrap_or(shipyard::Workload::new(stage));

        self.workloads.insert(stage, old_workload.merge(workload));

        self
    }

    pub fn add_plugin<T: Plugin>(mut self, workload: T) -> Self {
        log::trace!("Adding plugin '{}'", std::any::type_name::<T>());
        workload.build(&mut self);

        self
    }

    pub fn build(self) {
        // Add workloads to world
        self.workloads
            .into_iter()
            .for_each(|(_, workload)| workload.add_to_world(&self.world).unwrap());

        // Print debug data
        let data = self.world.workloads_info().0.iter().fold(
            String::from("Building workloads. Registered Stages and functions:"),
            |acc, (name, workload_info)| {
                let acc = format!("{}\n{}", acc, name);

                workload_info
                    .batch_info
                    .iter()
                    .fold(acc, |acc, batch_info| {
                        batch_info
                            .systems()
                            .fold(acc, |acc, system| format!("{}\n    {}", acc, system.name))
                    })
            },
        );

        log::debug!("{data}");

        // Make sure all workloads exist in world, even if empty
        enum_iterator::all::<Stages>()
            .into_iter()
            .for_each(
                |stage| match shipyard::Workload::new(stage).add_to_world(&self.world) {
                    Ok(_) | Err(shipyard::error::AddWorkload::AlreadyExists) => {}
                    Err(e) => panic!("{e}"),
                },
            );
    }
}

pub trait Plugin {
    fn build(&self, workload_builder: &mut WorkloadBuilder);
}

//====================================================================
