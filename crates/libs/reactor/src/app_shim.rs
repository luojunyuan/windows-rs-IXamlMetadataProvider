use std::cell::RefCell;
use windows_core::*;

use super::bindings::*;

implement_decl! {
    impl ReactorApplicationOverrides as pub ReactorApplicationOverrides_Impl: [IApplicationOverrides, IXamlMetadataProvider]
}

type XamlMetadataProviderFactory = Box<dyn Fn() -> Result<IInspectable>>;

thread_local! {
    static XAML_METADATA_PROVIDER_FACTORIES: RefCell<Vec<XamlMetadataProviderFactory>> =
        const { RefCell::new(Vec::new()) };
}

pub struct ReactorApplicationOverrides {
    controls_provider: RefCell<Option<XamlControlsXamlMetaDataProvider>>,
    external_providers: RefCell<Option<Vec<IXamlMetadataProvider>>>,
    on_launched: RefCell<Option<Box<dyn FnOnce() -> Result<()>>>>,
}

impl ReactorApplicationOverrides {
    fn new(on_launched: Box<dyn FnOnce() -> Result<()>>) -> Self {
        Self {
            controls_provider: RefCell::new(None),
            external_providers: RefCell::new(None),
            on_launched: RefCell::new(Some(on_launched)),
        }
    }

    fn provider(&self) -> Result<XamlControlsXamlMetaDataProvider> {
        if let Some(p) = self.controls_provider.borrow().as_ref() {
            return Ok(p.clone());
        }
        let p = XamlControlsXamlMetaDataProvider::new()?;
        *self.controls_provider.borrow_mut() = Some(p.clone());
        Ok(p)
    }

    fn external_providers(&self) -> Result<Vec<IXamlMetadataProvider>> {
        if let Some(providers) = self.external_providers.borrow().as_ref() {
            return Ok(providers.clone());
        }

        let providers = XAML_METADATA_PROVIDER_FACTORIES.with(|factories| {
            factories
                .borrow()
                .iter()
                .map(|factory| factory()?.cast())
                .collect::<Result<Vec<IXamlMetadataProvider>>>()
        })?;
        *self.external_providers.borrow_mut() = Some(providers.clone());
        Ok(providers)
    }
}

pub(crate) fn register_xaml_metadata_provider_factory<F>(factory: F)
where
    F: Fn() -> Result<IInspectable> + 'static,
{
    XAML_METADATA_PROVIDER_FACTORIES.with(|factories| {
        factories.borrow_mut().push(Box::new(factory));
    });
}

impl IApplicationOverrides_Impl for ReactorApplicationOverrides_Impl {
    fn OnLaunched(&self, _args: windows_core::Ref<LaunchActivatedEventArgs>) -> Result<()> {
        if let Some(cb) = self.on_launched.borrow_mut().take() {
            cb()?;
        }
        Ok(())
    }
}

impl IXamlMetadataProvider_Impl for ReactorApplicationOverrides_Impl {
    fn GetXamlType(&self, r#type: &TypeName) -> Result<IXamlType> {
        let provider: IXamlMetadataProvider = self.provider()?.cast()?;
        match provider.GetXamlType(r#type) {
            Ok(xaml_type) => Ok(xaml_type),
            Err(primary_error) => {
                for provider in self.external_providers()? {
                    if let Ok(xaml_type) = provider.GetXamlType(r#type) {
                        return Ok(xaml_type);
                    }
                }
                Err(primary_error)
            }
        }
    }

    fn GetXamlTypeByFullName(&self, full_name: &windows_core::HSTRING) -> Result<IXamlType> {
        let provider: IXamlMetadataProvider = self.provider()?.cast()?;
        let full_name = full_name.to_string_lossy();
        match provider.GetXamlTypeByFullName(&full_name) {
            Ok(xaml_type) => Ok(xaml_type),
            Err(primary_error) => {
                for provider in self.external_providers()? {
                    if let Ok(xaml_type) = provider.GetXamlTypeByFullName(&full_name) {
                        return Ok(xaml_type);
                    }
                }
                Err(primary_error)
            }
        }
    }

    fn GetXmlnsDefinitions(&self) -> Result<windows_core::Array<XmlnsDefinition>> {
        let provider: IXamlMetadataProvider = self.provider()?.cast()?;
        provider.GetXmlnsDefinitions()
    }
}

pub(crate) fn create_reactor_application(
    on_launched: Box<dyn FnOnce() -> Result<()>>,
) -> Result<Application> {
    Application::compose(ReactorApplicationOverrides::new(on_launched))
}

pub(crate) fn install_xaml_controls_resources(app: &Application) -> Result<()> {
    let controls = XamlControlsResources::new()?;
    let as_rd: ResourceDictionary = controls.cast()?;
    let resources = app.get_Resources()?;
    let merged = resources.get_MergedDictionaries()?;
    merged.Append(&as_rd)?;
    Ok(())
}
