use leptos::*;
use leptos_meta::*;
use leptos_router::*;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| view! { <App/> })
}

#[component]
fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/corelink-web.css"/>
        <Title text="CoreLink Dashboard"/>
        <Router>
            <main>
                <Routes>
                    <Route path="" view=HomePage/>
                    <Route path="/nodes" view=NodesPage/>
                    <Route path="/network" view=NetworkPage/>
                    <Route path="/storage" view=StoragePage/>
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    view! {
        <div class="container">
            <h1>"CoreLink Dashboard"</h1>
            <div class="stats">
                <StatCard title="Active Nodes" value="0"/>
                <StatCard title="Network Health" value="100%"/>
                <StatCard title="Data Stored" value="0 GB"/>
            </div>
            <nav class="nav-links">
                <A href="/nodes">"Nodes"</A>
                <A href="/network">"Network Topology"</A>
                <A href="/storage">"Storage"</A>
            </nav>
        </div>
    }
}

#[component]
fn StatCard(title: &'static str, value: &'static str) -> impl IntoView {
    view! {
        <div class="stat-card">
            <h3>{title}</h3>
            <p class="stat-value">{value}</p>
        </div>
    }
}

#[component]
fn NodesPage() -> impl IntoView {
    let (nodes, set_nodes) = create_signal(vec![]);

    view! {
        <div class="container">
            <h1>"Network Nodes"</h1>
            <button on:click=move |_| {
                set_nodes.update(|n| n.push(format!("Node {}", n.len() + 1)));
            }>
                "Refresh Nodes"
            </button>
            <div class="nodes-list">
                <For
                    each=move || nodes.get()
                    key=|node| node.clone()
                    children=move |node| {
                        view! {
                            <div class="node-card">
                                <p>{node}</p>
                            </div>
                        }
                    }
                />
            </div>
        </div>
    }
}

#[component]
fn NetworkPage() -> impl IntoView {
    view! {
        <div class="container">
            <h1>"Network Topology"</h1>
            <div class="topology-view">
                <p>"Network visualization will appear here"</p>
            </div>
        </div>
    }
}

#[component]
fn StoragePage() -> impl IntoView {
    view! {
        <div class="container">
            <h1>"Distributed Storage"</h1>
            <div class="storage-view">
                <p>"Storage management interface"</p>
            </div>
        </div>
    }
}
