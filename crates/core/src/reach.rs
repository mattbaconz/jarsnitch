//! Call graph over the top-level jar's own classes, and which methods the plugin's entry
//! points can reach. It errs toward "reached": when the bytecode leaves a call open
//! (virtual dispatch, callbacks from the server, reflection by name), every possible
//! target counts as reached. A method marked unreached has no call path that jarsnitch
//! can see; code that loads classes by names built at runtime can still get there.

use std::cell::OnceCell;
use std::collections::{HashMap, HashSet, VecDeque};

use crate::report::{Step, Via};

/// No parent: an entry point.
const NONE: u32 = u32::MAX;

/// Method name id and descriptor id, packed.
pub(crate) type Sig = u64;

/// Methods the JVM and the standard library call on any object.
const OBJECT_METHODS: [(&str, &str); 4] =
    [("toString", "()Ljava/lang/String;"), ("hashCode", "()I"), ("equals", "(Ljava/lang/Object;)Z"), ("finalize", "()V")];
/// Hierarchy walks stop after this many types; real hierarchies are far smaller.
const WALK_LIMIT: usize = 256;

#[derive(Default)]
pub(crate) struct Names {
    ids: HashMap<Box<str>, u32>,
    list: Vec<Box<str>>,
}

impl Names {
    pub fn id(&mut self, s: &str) -> u32 {
        if let Some(&i) = self.ids.get(s) {
            return i;
        }
        let i = self.list.len() as u32;
        self.ids.insert(s.into(), i);
        self.list.push(s.into());
        i
    }
    pub fn get(&self, s: &str) -> Option<u32> {
        self.ids.get(s).copied()
    }
    fn str(&self, i: u32) -> &str {
        self.list.get(i as usize).map_or("", |s| s)
    }
}

/// Something a method's code does that matters for reachability.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Ref {
    /// A call or method handle: owner class and signature.
    Call(u32, Sig),
    /// `new` on a class.
    New(u32),
    /// Static field access: the class gets initialized.
    Init(u32),
    /// A class literal, a constant class name passed to reflection, or a string that
    /// names a class: anything in that class may run.
    Reflect(u32),
    /// Loads classes by a name built at runtime, or defines classes from bytes.
    Dynamic,
    /// Lists a class's methods by reflection, as command and event frameworks do.
    Enumerate,
}

struct Class {
    sup: Option<u32>,
    ifaces: Vec<u32>,
    methods: Vec<u32>,
}

struct Method {
    class: u32,
    sig: Sig,
    is_static: bool,
    /// Public or protected: can override a method of a type outside the jar.
    visible: bool,
    has_code: bool,
    /// Has runtime-visible annotations (`@EventHandler`, `@Subscribe`, `@Command`): a
    /// framework may call it by reflection.
    annotated: bool,
    refs: Vec<Ref>,
}

#[derive(Default)]
pub(crate) struct Graph {
    pub names: Names,
    classes: HashMap<u32, Class>,
    methods: Vec<Method>,
    by_key: HashMap<(u32, Sig), u32>,
    by_sig: HashMap<Sig, Vec<u32>>,
    /// Every supertype of each class, in the jar or not; built on first use.
    supertypes: OnceCell<HashMap<u32, Vec<u32>>>,
}

/// What the entry points reach.
pub(crate) struct Reach {
    reached: Vec<bool>,
    /// The method whose code first led to each reached method, NONE for entry points.
    /// The work list runs first in, first out, so these give short paths.
    parent: Vec<u32>,
    via: Vec<Via>,
    live: HashSet<u32>,
    pub dynamic: bool,
}

impl Graph {
    pub fn sig(&mut self, name: &str, desc: &str) -> Sig {
        (u64::from(self.names.id(name)) << 32) | u64::from(self.names.id(desc))
    }

    fn known_sig(&self, name: &str, desc: &str) -> Option<Sig> {
        Some((u64::from(self.names.get(name)?) << 32) | u64::from(self.names.get(desc)?))
    }

    pub fn class(&mut self, name: &str, sup: Option<&str>, ifaces: &[&str]) -> u32 {
        let id = self.names.id(name);
        let sup = sup.map(|s| self.names.id(s));
        let ifaces: Vec<u32> = ifaces.iter().map(|i| self.names.id(i)).collect();
        // A second copy (a multi-release jar's versioned class, a duplicate entry) adds
        // its methods to the first.
        self.classes.entry(id).or_insert(Class { sup, ifaces, methods: Vec::new() });
        id
    }

    #[allow(clippy::too_many_arguments)]
    pub fn method(&mut self, class: u32, name: &str, desc: &str, is_static: bool, visible: bool, has_code: bool, annotated: bool) -> u32 {
        let sig = self.sig(name, desc);
        if let Some(&m) = self.by_key.get(&(class, sig)) {
            return m;
        }
        let m = self.methods.len() as u32;
        self.methods.push(Method { class, sig, is_static, visible, has_code, annotated, refs: Vec::new() });
        self.by_key.insert((class, sig), m);
        self.by_sig.entry(sig).or_default().push(m);
        if let Some(c) = self.classes.get_mut(&class) {
            c.methods.push(m);
        }
        m
    }

    pub fn add(&mut self, m: u32, r: Ref) {
        if let Some(method) = self.methods.get_mut(m as usize) {
            method.refs.push(r);
        }
    }

    pub fn end_method(&mut self, m: u32) {
        if let Some(method) = self.methods.get_mut(m as usize) {
            method.refs.sort_unstable();
            method.refs.dedup();
        }
    }

    pub fn has_class(&self, name: &str) -> bool {
        self.names.get(name).is_some_and(|id| self.classes.contains_key(&id))
    }

    pub fn method_count(&self) -> usize {
        self.methods.len()
    }

    /// Location and `name(desc)` of a method, as evidence spells them.
    pub fn describe(&self, m: u32) -> (String, String) {
        let method = &self.methods[m as usize];
        let (name, desc) = ((method.sig >> 32) as u32, method.sig as u32);
        (self.names.str(method.class).to_string(), format!("{}{}", self.names.str(name), self.names.str(desc)))
    }

    /// Method id for evidence at `location` in `method` (`name(desc)`).
    pub fn method_at(&self, location: &str, method: &str) -> Option<u32> {
        let class = self.names.get(location)?;
        let at = method.find('(')?;
        let sig = self.known_sig(&method[..at], &method[at..])?;
        self.by_key.get(&(class, sig)).copied()
    }

    fn supertypes(&self, c: u32) -> &[u32] {
        let all = self.supertypes.get_or_init(|| {
            let mut all = HashMap::with_capacity(self.classes.len());
            for &c in self.classes.keys() {
                let mut out: Vec<u32> = Vec::new();
                let mut i = 0;
                let mut next = Some(c);
                while let Some(x) = next {
                    if let Some(class) = self.classes.get(&x) {
                        for &p in class.sup.iter().chain(&class.ifaces) {
                            if p != c && !out.contains(&p) && out.len() < WALK_LIMIT {
                                out.push(p);
                            }
                        }
                    }
                    next = out.get(i).copied();
                    i += 1;
                }
                all.insert(c, out);
            }
            all
        });
        all.get(&c).map_or(&[], Vec::as_slice)
    }

    fn is_subtype(&self, c: u32, of: u32) -> bool {
        c == of || self.supertypes(c).contains(&of)
    }

    /// The method a call on `owner` with `sig` names: declared in `owner`, a supertype
    /// or, as a default method, an interface.
    fn resolve(&self, owner: u32, sig: Sig) -> Option<u32> {
        if let Some(&m) = self.by_key.get(&(owner, sig)) {
            return Some(m);
        }
        self.classes.get(&owner)?;
        self.supertypes(owner).iter().find_map(|&c| self.by_key.get(&(c, sig)).copied())
    }

    pub fn sig_of(&self, m: u32) -> Sig {
        self.methods[m as usize].sig
    }

    /// The methods a method's calls with one of `sigs` name directly, or the only
    /// implementation in the jar when the named method is abstract.
    pub fn callees(&self, m: u32, sigs: &HashSet<Sig>) -> Vec<u32> {
        let mut out = Vec::new();
        for r in &self.methods[m as usize].refs {
            let Ref::Call(owner, sig) = *r else { continue };
            if !sigs.contains(&sig) {
                continue;
            }
            let target = self.resolve(owner, sig).filter(|&t| self.methods[t as usize].has_code).or_else(|| {
                self.classes.get(&owner)?;
                let mut impls = self.by_sig.get(&sig)?.iter().copied().filter(|&t| {
                    let t = &self.methods[t as usize];
                    t.has_code && self.is_subtype(t.class, owner)
                });
                let first = impls.next()?;
                impls.next().is_none().then_some(first)
            });
            if let Some(t) = target
                && t != m
                && !out.contains(&t)
            {
                out.push(t);
            }
        }
        out
    }

    /// Everything reachable from `roots`, classes the server or a service loader
    /// creates, and `whole`, classes whose every method may run.
    pub fn reach(&self, roots: &[String], whole: &[String]) -> Reach {
        let object_sigs = OBJECT_METHODS.iter().filter_map(|(n, d)| self.known_sig(n, d)).collect();
        let mut s = State {
            g: self,
            reached: vec![false; self.methods.len()],
            parent: vec![NONE; self.methods.len()],
            via: vec![Via::Entry; self.methods.len()],
            cur: NONE,
            work: VecDeque::new(),
            inited: HashSet::new(),
            instantiated: HashSet::new(),
            instances: HashMap::new(),
            virtual_calls: HashMap::new(),
            seen_calls: HashSet::new(),
            enumerate: false,
            enumerator: NONE,
            dynamic: false,
            object_sigs,
            object: self.names.get("java/lang/Object"),
            clinit: self.known_sig("<clinit>", "()V"),
            init: self.names.get("<init>"),
        };
        for e in roots {
            let Some(id) = self.names.get(e) else { continue };
            s.instantiate(id);
            let constructors: Vec<u32> = self.classes.get(&id).map_or(Vec::new(), |c| {
                c.methods.iter().copied().filter(|&m| s.is_constructor(self.methods[m as usize].sig)).collect()
            });
            for m in constructors {
                s.reach(m, Via::Entry);
            }
        }
        for e in whole {
            if let Some(id) = self.names.get(e) {
                s.everything(id);
            }
        }
        while let Some(m) = s.work.pop_front() {
            s.cur = m;
            for &r in &self.methods[m as usize].refs {
                s.apply(r);
            }
        }
        let mut live = s.inited;
        live.extend(s.instantiated);
        for (m, &r) in s.reached.iter().enumerate() {
            if r {
                live.insert(self.methods[m].class);
            }
        }
        Reach { reached: s.reached, parent: s.parent, via: s.via, live, dynamic: s.dynamic }
    }
}

impl Reach {
    /// Whether code at `location` (in `method`, or anywhere in the class) can run.
    /// None when `location` isn't one of the jar's own classes.
    pub fn at(&self, g: &Graph, location: &str, method: Option<&str>) -> Option<bool> {
        let class = g.names.get(location).filter(|c| g.classes.contains_key(c))?;
        Some(match method {
            Some(m) => g.method_at(location, m).is_none_or(|m| self.reached[m as usize]),
            None => self.live.contains(&class),
        })
    }

    pub fn reached_count(&self) -> usize {
        self.reached.iter().filter(|&&r| r).count()
    }

    /// One call path from an entry point to code at `location` (in `method`, or in any
    /// method of the class), entry point first. Empty when that code isn't reached.
    pub fn path(&self, g: &Graph, location: &str, method: Option<&str>) -> Vec<Step> {
        let target = match method {
            Some(m) => g.method_at(location, m).filter(|&m| self.reached[m as usize]),
            // Anywhere in the class: its shortest path, constructors and static
            // initializers only when nothing else in the class is reached.
            None => g.names.get(location).and_then(|c| g.classes.get(&c)).and_then(|c| {
                c.methods.iter().copied().filter(|&m| self.reached[m as usize]).min_by_key(|&m| {
                    let special = g.names.str((g.methods[m as usize].sig >> 32) as u32).starts_with('<');
                    (special, self.depth(m))
                })
            }),
        };
        let mut steps = Vec::new();
        let mut next = target.unwrap_or(NONE);
        // A parent is always reached before its child, so the chain ends at an entry point.
        while next != NONE && steps.len() < self.parent.len() {
            let (location, method) = g.describe(next);
            steps.push(Step { location, method, via: self.via[next as usize] });
            next = self.parent[next as usize];
        }
        steps.reverse();
        steps
    }

    fn depth(&self, mut m: u32) -> usize {
        let mut n = 0;
        while m != NONE && n < self.parent.len() {
            m = self.parent[m as usize];
            n += 1;
        }
        n
    }
}

struct State<'g> {
    g: &'g Graph,
    reached: Vec<bool>,
    parent: Vec<u32>,
    via: Vec<Via>,
    /// The method whose code is being applied; NONE while the entry points are set up.
    cur: u32,
    work: VecDeque<u32>,
    inited: HashSet<u32>,
    instantiated: HashSet<u32>,
    /// The instantiated classes below each type, the type itself included.
    instances: HashMap<u32, Vec<u32>>,
    /// Owner and calling method of the virtual calls seen so far, by signature.
    virtual_calls: HashMap<Sig, Vec<(u32, u32)>>,
    seen_calls: HashSet<(Sig, u32)>,
    enumerate: bool,
    /// The method that first lists a class's methods by reflection.
    enumerator: u32,
    dynamic: bool,
    object_sigs: Vec<Sig>,
    object: Option<u32>,
    clinit: Option<Sig>,
    /// Name id of `<init>`.
    init: Option<u32>,
}

impl State<'_> {
    fn is_constructor(&self, sig: Sig) -> bool {
        Some((sig >> 32) as u32) == self.init
    }

    /// A call on `owner` may land on a method declared in `t`, an instantiated type.
    fn dispatches(&self, owner: u32, t: u32) -> bool {
        !self.g.classes.contains_key(&owner) || self.g.is_subtype(t, owner) || self.g.is_subtype(owner, t)
    }

    /// A call on `owner` may also land on `t`'s method through an instantiated class
    /// below both that inherits it, as when `AbstractModule.start` implements `Module`
    /// for a `FooModule extends AbstractModule implements Module`.
    fn lands(&self, owner: u32, t: u32) -> bool {
        self.dispatches(owner, t) || self.instances.get(&owner).is_some_and(|cs| cs.iter().any(|&c| self.g.is_subtype(c, t)))
    }

    /// The method that made the first virtual call seen so far that could land on the
    /// method with `sig` declared in `t`. The same test as `apply`, so the result
    /// doesn't depend on whether the call or the `new` comes first.
    fn caller(&self, t: u32, sig: Sig) -> Option<u32> {
        self.virtual_calls.get(&sig)?.iter().find(|&&(o, _)| self.lands(o, t)).map(|&(_, caller)| caller)
    }

    fn reach(&mut self, m: u32, how: Via) {
        self.reach_from(m, how, self.cur);
    }

    /// `parent` is the method whose code leads to `m`, NONE for an entry point.
    fn reach_from(&mut self, m: u32, how: Via, parent: u32) {
        if let Some(r) = self.reached.get_mut(m as usize)
            && !*r
        {
            *r = true;
            self.parent[m as usize] = parent;
            self.via[m as usize] = if parent == NONE { Via::Entry } else { how };
            self.work.push_back(m);
        }
    }

    /// Class initialization runs the static initializer, superclasses first.
    fn init(&mut self, c: u32) {
        let mut next = Some(c);
        let mut steps = 0;
        while let Some(x) = next {
            steps += 1;
            if steps > WALK_LIMIT || !self.g.classes.contains_key(&x) || !self.inited.insert(x) {
                return;
            }
            if let Some(m) = self.clinit.and_then(|s| self.g.by_key.get(&(x, s)).copied()) {
                self.reach(m, Via::StaticInit);
            }
            next = self.g.classes[&x].sup;
        }
    }

    fn instantiate(&mut self, c: u32) {
        if !self.g.classes.contains_key(&c) || !self.instantiated.insert(c) {
            return;
        }
        self.init(c);
        let types: Vec<u32> = std::iter::once(c).chain(self.g.supertypes(c).iter().copied()).collect();
        // A type from outside the jar (Listener, Runnable, CommandExecutor, ClassLoader)
        // means the server or the JVM may call any method the class overrides.
        let callbacks = types.iter().any(|t| !self.g.classes.contains_key(t) && Some(*t) != self.object);
        // Listed before the methods, so `caller` sees this class. A method listing runs
        // `instantiate` again on classes already listed.
        if !self.instances.get(&c).is_some_and(|cs| cs.contains(&c)) {
            for &t in &types {
                self.instances.entry(t).or_default().push(c);
            }
        }
        for t in types {
            let Some(class) = self.g.classes.get(&t) else { continue };
            for &m in &class.methods {
                let method = &self.g.methods[m as usize];
                if self.reached[m as usize] || (method.is_static && !self.enumerate) {
                    continue;
                }
                // The step before is the method that creates the object, makes the call
                // or lists the methods.
                let (how, parent) = if method.is_static {
                    (Via::Reflection, self.enumerator)
                } else if (callbacks && method.visible) || method.annotated || self.object_sigs.contains(&method.sig) {
                    (Via::Callback, self.cur)
                } else if let Some(caller) = self.caller(t, method.sig) {
                    (Via::Call, caller)
                } else if self.enumerate {
                    (Via::Reflection, self.enumerator)
                } else {
                    continue;
                };
                self.reach_from(m, how, parent);
            }
        }
    }

    /// The class may be loaded and used by reflection: every method counts.
    fn everything(&mut self, c: u32) {
        let Some(class) = self.g.classes.get(&c) else { return };
        let methods = class.methods.clone();
        self.instantiate(c);
        for m in methods {
            self.reach(m, Via::Reflection);
        }
    }

    fn apply(&mut self, r: Ref) {
        match r {
            Ref::Call(owner, sig) => {
                let target = self.g.resolve(owner, sig);
                if let Some(t) = target {
                    self.reach(t, Via::Call);
                    self.init(self.g.methods[t as usize].class);
                }
                // Constructors and static methods don't dispatch on the receiver.
                if self.is_constructor(sig)
                    || target.is_some_and(|t| self.g.methods[t as usize].is_static)
                    || !self.seen_calls.insert((sig, owner))
                {
                    return;
                }
                self.virtual_calls.entry(sig).or_default().push((owner, self.cur));
                let Some(candidates) = self.g.by_sig.get(&sig) else { return };
                for &m in candidates {
                    let method = &self.g.methods[m as usize];
                    if !self.reached[m as usize]
                        && !method.is_static
                        && self.instances.contains_key(&method.class)
                        && self.lands(owner, method.class)
                    {
                        self.reach(m, Via::Call);
                    }
                }
            }
            Ref::New(c) => self.instantiate(c),
            Ref::Init(c) => self.init(c),
            Ref::Reflect(c) => self.everything(c),
            Ref::Dynamic => self.dynamic = true,
            Ref::Enumerate => {
                if !self.enumerate {
                    self.enumerate = true;
                    self.enumerator = self.cur;
                    // Sorted: queue order decides each method's parent, and ids follow
                    // the jar's own order.
                    let mut classes: Vec<u32> = self.instantiated.drain().collect();
                    classes.sort_unstable();
                    for c in classes {
                        self.instantiate(c);
                    }
                }
            }
        }
    }
}
