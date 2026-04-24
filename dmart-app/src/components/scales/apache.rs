use leptos::prelude::*;
use dmart_shared::models::*;
use dmart_shared::scales::*;

#[component]
pub fn ApacheIIScale(
    data: RwSignal<ApacheIIData>,
) -> impl IntoView {
    let breakdown = Memo::new(move |_| apache_ii_breakdown(&data.get()));
    let score = Memo::new(move |_| calculate_apache_ii_score(&data.get()));
    let severity = Memo::new(move |_| SeverityLevel::from_score(score.get()));
    let mortality = Memo::new(move |_| mortality_risk(score.get()));

    let severity_color = move || {
        match severity.get() {
            SeverityLevel::Bajo => "var(--uci-low)",
            SeverityLevel::Moderado => "var(--uci-moderate)",
            SeverityLevel::Severo => "var(--uci-severe)",
            SeverityLevel::Critico => "var(--uci-critical)",
        }
    };

    let temp_val = Signal::derive(move || data.get().temperatura);
    let temp_set = Callback::new(move |v| data.update(|d| d.temperatura = v));
    
    let pam_val = Signal::derive(move || data.get().presion_arterial_media);
    let pam_set = Callback::new(move |v| data.update(|d| d.presion_arterial_media = v));
    
    let fc_val = Signal::derive(move || data.get().frecuencia_cardiaca);
    let fc_set = Callback::new(move |v| data.update(|d| d.frecuencia_cardiaca = v));
    
    let fr_val = Signal::derive(move || data.get().frecuencia_respiratoria);
    let fr_set = Callback::new(move |v| data.update(|d| d.frecuencia_respiratoria = v));
    
    let fio2_val = Signal::derive(move || data.get().fio2);
    let fio2_set = Callback::new(move |v| data.update(|d| d.fio2 = v));
    
    let pao2_val = Signal::derive(move || data.get().pao2.unwrap_or(80.0));
    let pao2_set = Callback::new(move |v| data.update(|d| d.pao2 = Some(v)));
    
    let ph_val = Signal::derive(move || data.get().ph_arterial);
    let ph_set = Callback::new(move |v| data.update(|d| d.ph_arterial = v));
    
    let sodio_val = Signal::derive(move || data.get().sodio_serico);
    let sodio_set = Callback::new(move |v| data.update(|d| d.sodio_serico = v));
    
    let potasio_val = Signal::derive(move || data.get().potasio_serico);
    let potasio_set = Callback::new(move |v| data.update(|d| d.potasio_serico = v));
    
    let creatinina_val = Signal::derive(move || data.get().creatinina);
    let creatinina_set = Callback::new(move |v| data.update(|d| d.creatinina = v));
    
    let hematocrito_val = Signal::derive(move || data.get().hematocrito);
    let hematocrito_set = Callback::new(move |v| data.update(|d| d.hematocrito = v));
    
    let leucocitos_val = Signal::derive(move || data.get().leucocitos);
    let leucocitos_set = Callback::new(move |v| data.update(|d| d.leucocitos = v));

    // Paciente
    let gcs_total = Signal::derive(move || data.get().gcs_ojos + data.get().gcs_verbal + data.get().gcs_motor);
    let edad_val = Signal::derive(move || data.get().edad as f32);
    let edad_set = Callback::new(move |v| data.update(|d| d.edad = v as u8));
    let hep_val = Signal::derive(move || data.get().insuficiencia_hepatica);
    let hep_set = Callback::new(move |v| data.update(|d| d.insuficiencia_hepatica = v));
    let cv_val = Signal::derive(move || data.get().cardiovascular_severa);
    let cv_set = Callback::new(move |v| data.update(|d| d.cardiovascular_severa = v));
    let resp_val = Signal::derive(move || data.get().insuficiencia_respiratoria);
    let resp_set = Callback::new(move |v| data.update(|d| d.insuficiencia_respiratoria = v));
    let renal_val = Signal::derive(move || data.get().insuficiencia_renal);
    let renal_set = Callback::new(move |v| data.update(|d| d.insuficiencia_renal = v));
    let immuno_val = Signal::derive(move || data.get().inmunocomprometido);
    let immuno_set = Callback::new(move |v| data.update(|d| d.inmunocomprometido = v));
    let cirugia_val = Signal::derive(move || data.get().cirugia_no_operado);
    let cirugia_set = Callback::new(move |v| data.update(|d| d.cirugia_no_operado = v));

    let gcs_ojos_val = Signal::derive(move || data.get().gcs_ojos);
    let gcs_ojos_set = Callback::new(move |v| data.update(|d| d.gcs_ojos = v));
    let gcs_verbal_val = Signal::derive(move || data.get().gcs_verbal);
    let gcs_verbal_set = Callback::new(move |v| data.update(|d| d.gcs_verbal = v));
    let gcs_motor_val = Signal::derive(move || data.get().gcs_motor);
    let gcs_motor_set = Callback::new(move |v| data.update(|d| d.gcs_motor = v));

    view! {
        <div class="glass-card p-4" style="background:var(--uci-card); border:1px solid var(--uci-border);">
            <div class="flex items-center gap-3 mb-4">
                <div class="w-10 h-10 rounded-lg flex items-center justify-center" 
                     style="background:linear-gradient(135deg, var(--uci-accent), var(--uci-accent2)); color:white;">
                    <i class="fa-solid fa-heart-pulse"></i>
                </div>
                <div>
                    <h3 class="text-xl font-black uppercase" style="color:var(--uci-text);">"APACHE II"</h3>
                    <p class="text-[10px] font-bold" style="color:var(--uci-muted);">"Fisiología Aguda"</p>
                </div>
            </div>

            <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-2">
                <SliderField label="Temp (°C)" icon="fa-temperature-half" min=30.0 max=45.0 step=0.1 color="#EF4444" 
                    value=temp_val on_change=temp_set />
                <SliderField label="PAM" icon="fa-gauge-high" min=20.0 max=200.0 step=1.0 color="#F97316" 
                    value=pam_val on_change=pam_set />
                <SliderField label="FC" icon="fa-heartbeat" min=20.0 max=220.0 step=1.0 color="#EC4899" 
                    value=fc_val on_change=fc_set />
                <SliderField label="FR" icon="fa-wind" min=5.0 max=60.0 step=1.0 color="#8B5CF6" 
                    value=fr_val on_change=fr_set />
                <SliderField label="FiO2" icon="fa-wind" min=0.21 max=1.0 step=0.01 color="#06B6D4" 
                    value=fio2_val on_change=fio2_set />
                <SliderField label="PaO2" icon="fa-lungs" min=40.0 max=500.0 step=1.0 color="#10B981" 
                    value=pao2_val on_change=pao2_set />
                <SliderField label="pH" icon="fa-vial" min=6.8 max=7.8 step=0.01 color="#14B8A6" 
                    value=ph_val on_change=ph_set />
                <SliderField label="Sodio" icon="fa-flask" min=110.0 max=180.0 step=1.0 color="#F59E0B" 
                    value=sodio_val on_change=sodio_set />
                <SliderField label="Potasio" icon="fa-flask" min=1.0 max=10.0 step=0.1 color="#8B5CF6" 
                    value=potasio_val on_change=potasio_set />
                <SliderField label="Creatinina" icon="fa-kidneys" min=0.1 max=15.0 step=0.1 color="#DC2626" 
                    value=creatinina_val on_change=creatinina_set />
                <SliderField label="Hematocrito" icon="fa-droplet" min=10.0 max=65.0 step=1.0 color="#DC2626" 
                    value=hematocrito_val on_change=hematocrito_set />
                <SliderField label="Leucocitos" icon="fa-microscope" min=0.1 max=50.0 step=0.1 color="#6366F1" 
                    value=leucocitos_val on_change=leucocitos_set />
            </div>

            <div class="mt-4 p-3 rounded-lg" style="background:var(--uci-surface); border:1px solid var(--uci-border);">
                <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
                    <SliderField label="Edad" icon="fa-user" min=18.0 max=120.0 step=1.0 color="#6366F1" 
                        value=edad_val on_change=edad_set />
                </div>
                <h4 class="text-xs font-bold uppercase mt-3 mb-2" style="color:var(--uci-muted);">"Enfermedades Crónicas"</h4>
                <div class="grid grid-cols-1 sm:grid-cols-2 gap-2">
                    <ToggleField label="Insuficiencia Hepática" color="#6366F1" value=hep_val on_change=hep_set />
                    <ToggleField label="Cardiovascular Severa" color="#6366F1" value=cv_val on_change=cv_set />
                    <ToggleField label="Insuficiencia Respiratoria" color="#6366F1" value=resp_val on_change=resp_set />
                    <ToggleField label="Insuficiencia Renal" color="#6366F1" value=renal_val on_change=renal_set />
                    <ToggleField label="Inmunocomprometido" color="#6366F1" value=immuno_val on_change=immuno_set />
                    <ToggleField label="Cirugía No Operado/Emergencia" color="#6366F1" value=cirugia_val on_change=cirugia_set />
                </div>
            </div>

            <div class="mt-4 pt-3 border-t" style="border-color:var(--uci-border);">
                <h4 class="text-xs font-bold uppercase mb-2" style="color:var(--uci-muted);">"Glasgow (GCS)"</h4>
                <div class="grid grid-cols-3 gap-2">
                    <GcsSlider label="Ojos" color="#10B981" min=1 max=4 
                        value=gcs_ojos_val on_change=gcs_ojos_set />
                    <GcsSlider label="Verbal" color="#F59E0B" min=1 max=5 
                        value=gcs_verbal_val on_change=gcs_verbal_set />
                    <GcsSlider label="Motor" color="#8B5CF6" min=1 max=6 
                        value=gcs_motor_val on_change=gcs_motor_set />
                </div>
                <div class="mt-2 text-center p-2 rounded-lg" style="background:linear-gradient(135deg, var(--uci-accent), var(--uci-accent2)); color:white;">
                    <span class="text-sm font-bold">"GCS: "</span>
                    <span class="text-xl font-black">{move || gcs_total.get()}</span>
                    <span class="text-xs opacity-70">"/15"</span>
                </div>
            </div>

            <div class="mt-4 space-y-2">
                <div class="grid grid-cols-3 gap-2">
                    <div class="text-center p-3 rounded-lg" style="background:var(--uci-surface);">
                        <div class="text-[10px] font-bold uppercase" style="color:var(--uci-muted);">"APS"</div>
                        <div class="text-2xl font-black" style="color:#F97316;">{move || breakdown.get().aps_total}</div>
                    </div>
                    <div class="text-center p-3 rounded-lg" style="background:var(--uci-surface);">
                        <div class="text-[10px] font-bold uppercase" style="color:var(--uci-muted);">"Gravedad"</div>
                        <div class="text-2xl font-black" style=format!("color:{};", severity_color())>{move || format!("{} (+{})", score.get(), breakdown.get().edad_pts + breakdown.get().cronicas_pts)}</div>
                    </div>
                    <div class="text-center p-3 rounded-lg" style="background:var(--uci-surface);">
                        <div class="text-[10px] font-bold uppercase" style="color:var(--uci-muted);">"Mortalidad"</div>
                        <div class="text-2xl font-black" style=format!("color:{};", severity_color())>{move || format!("{:.1}%", mortality.get())}</div>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn ToggleField(
    label: &'static str,
    color: &'static str,
    value: Signal<bool>,
    on_change: Callback<bool>,
) -> impl IntoView {
    view! {
        <label class="flex items-center gap-2 cursor-pointer">
            <input 
                type="checkbox"
                class="w-4 h-4 rounded"
                prop:checked=value.get()
                on:change=move |ev| {
                    on_change.run(event_target_checked(&ev));
                }
            />
            <span class="text-xs font-bold" style=format!("color:{};", color)>{label}</span>
        </label>
    }
}

#[component]
fn SliderField(
    label: &'static str,
    icon: &'static str,
    min: f32,
    max: f32,
    step: f32,
    color: &'static str,
    value: Signal<f32>,
    on_change: Callback<f32>,
) -> impl IntoView {
    view! {
        <div class="p-2 rounded-lg" style="background:var(--uci-surface);">
            <div class="flex justify-between items-center mb-1">
                <label class="text-[10px] font-bold flex items-center gap-1" style="color:var(--uci-muted);">
                    <i class=format!("fa-solid {}", icon) style=format!("font-size:9px;color:{};", color)></i>
                    {label}
                </label>
                <span class="text-sm font-bold font-mono"
                      style=format!("color:{};", color)>
                    {move || format!("{:.1}", value.get())}
                </span>
            </div>
            <input 
                type="range" class="w-full h-1.5 rounded-lg appearance-none cursor-pointer"
                min=min max=max step=step
                prop:value=move || value.get()
                on:input=move |ev| {
                    if let Ok(v) = event_target_value(&ev).parse::<f32>() {
                        on_change.run(v);
                    }
                }
            />
        </div>
    }
}

#[component]
fn GcsSlider(
    label: &'static str,
    color: &'static str,
    min: u8,
    max: u8,
    value: Signal<u8>,
    on_change: Callback<u8>,
) -> impl IntoView {
    view! {
        <div class="text-center p-2 rounded-lg" style="background:var(--uci-surface);">
            <label class="text-[10px] font-bold block mb-1" style="color:var(--uci-muted);">{label}</label>
            <input 
                type="range" class="w-full h-1.5"
                min=min max=max step=1
                prop:value=move || value.get()
                on:input=move |ev| {
                    if let Ok(v) = event_target_value(&ev).parse::<u8>() {
                        on_change.run(v);
                    }
                }
            />
            <div class="text-xl font-black" style=format!("color:{};", color)>{move || value.get()}</div>
        </div>
    }
}