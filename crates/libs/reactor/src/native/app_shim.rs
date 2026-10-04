use super::bindings::*;
use std::cell::RefCell;
use windows_core::*;

implement_decl! {
    impl ReactorApplicationOverrides as pub ReactorApplicationOverrides_Impl:
        [IApplicationOverrides, IXamlMetadataProvider]
}

pub struct ReactorApplicationOverrides {
    controls_provider: RefCell<Option<XamlControlsXamlMetaDataProvider>>,
    additional_provider: Option<IXamlMetadataProvider>,
    on_launched: RefCell<Option<Box<dyn FnOnce() -> Result<()>>>>,
}

impl ReactorApplicationOverrides {
    fn new(
        on_launched: Box<dyn FnOnce() -> Result<()>>,
        additional_provider: Option<IUnknown>,
    ) -> Result<Self> {
        Ok(Self {
            controls_provider: RefCell::new(None),
            additional_provider: additional_provider
                .map(|provider| provider.cast())
                .transpose()?,
            on_launched: RefCell::new(Some(on_launched)),
        })
    }

    fn provider(&self) -> Result<XamlControlsXamlMetaDataProvider> {
        if let Some(provider) = self.controls_provider.borrow().as_ref() {
            return Ok(provider.clone());
        }
        let provider = XamlControlsXamlMetaDataProvider::new()?;
        *self.controls_provider.borrow_mut() = Some(provider.clone());
        Ok(provider)
    }
}

impl IApplicationOverrides_Impl for ReactorApplicationOverrides_Impl {
    fn OnLaunched(&self, _args: Ref<LaunchActivatedEventArgs>) -> Result<()> {
        if let Some(on_launched) = self.on_launched.borrow_mut().take() {
            on_launched()?;
        }
        Ok(())
    }
}

impl IXamlMetadataProvider_Impl for ReactorApplicationOverrides_Impl {
    fn GetXamlType(&self, r#type: &TypeName) -> Result<IXamlType> {
        if let Some(provider) = &self.additional_provider {
            if let Ok(xaml_type) = provider.GetXamlType(r#type) {
                return Ok(xaml_type);
            }
        }
        self.provider()?.GetXamlType(r#type)
    }

    fn GetXamlTypeByFullName(&self, full_name: &HSTRING) -> Result<IXamlType> {
        if let Some(provider) = &self.additional_provider {
            if let Ok(xaml_type) = provider.GetXamlTypeByFullName(&full_name.to_string_lossy()) {
                return Ok(xaml_type);
            }
        }
        self.provider()?
            .GetXamlTypeByFullName(&full_name.to_string_lossy())
    }

    fn GetXmlnsDefinitions(&self) -> Result<Array<XmlnsDefinition>> {
        let controls = self.provider()?.GetXmlnsDefinitions()?;
        let Some(provider) = &self.additional_provider else {
            return Ok(controls);
        };
        let additional = provider.GetXmlnsDefinitions()?;
        let mut definitions = Vec::with_capacity(additional.len() + controls.len());
        definitions.extend_from_slice(&additional);
        definitions.extend_from_slice(&controls);
        Ok(Array::from_slice(&definitions))
    }
}

pub(super) fn create_application(
    on_launched: Box<dyn FnOnce() -> Result<()>>,
    additional_provider: Option<IUnknown>,
) -> Result<Application> {
    Application::compose(ReactorApplicationOverrides::new(
        on_launched,
        additional_provider,
    )?)
}

pub(super) fn install_xaml_controls_resources(application: &Application) -> Result<()> {
    let controls = XamlControlsResources::new()?;
    let resources: ResourceDictionary = controls.cast()?;
    application
        .Resources()?
        .MergedDictionaries()?
        .Append(&resources)
}
