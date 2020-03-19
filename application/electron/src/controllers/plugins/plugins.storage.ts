import * as path from 'path';
import * as FS from '../../tools/fs';

import Logger from '../../tools/env.logger';
import ServicePaths from '../../services/service.paths';
import ControllerPluginRender from './plugin.controller.render';
import ServiceElectronService from '../../services/service.electron.state';
import ServiceEnv from '../../services/service.env';

import InstalledPlugin, { TConnectionFactory, IInstalledPluginInfo } from './plugin.installed';
import ControllerPluginStore, { IPluginReleaseInfo } from './plugins.store';

import { IPCMessages } from '../../services/service.electron';
import { CommonInterfaces } from '../../interfaces/interface.common';

export { InstalledPlugin, TConnectionFactory };

/**
 * @class ControllerPluginInstalled
 * @description Delivery default plugins into chipmunk folder
 */

export default class ControllerPluginsStorage {

    private _logger: Logger = new Logger('ControllerPluginsStorage');
    private _plugins: Map<string, InstalledPlugin> = new Map();
    private _store: ControllerPluginStore;
    private _installing: Map<string, string> = new Map();
    private _uninstalling: Map<string, string> = new Map();

    constructor(store: ControllerPluginStore) {
        this._store = store;
    }

    public load(): Promise<void> {
        return new Promise((resolve, reject) => {
            ServiceElectronService.logStateToRender(`Reading installed plugins...`);
            const pluginStorageFolder: string = ServicePaths.getPlugins();
            // Get all sub folders from plugins folder. Expecting: there are plugins folders
            FS.readFolders(pluginStorageFolder).then((folders: string[]) => {
                if (folders.length === 0) {
                    // No any plugins
                    this._logger.debug(`No any plugins were found. Target folder: ${pluginStorageFolder}`);
                    return resolve();
                }
                const toBeRemoved: InstalledPlugin[] = [];
                // Check each plugin folder and read package.json of render and process apps
                Promise.all(folders.map((folder: string) => {
                    const plugin: InstalledPlugin = new InstalledPlugin(folder, path.resolve(pluginStorageFolder, folder), this._store);
                    return plugin.read().then(() => {
                        this._plugins.set(plugin.getName(), plugin);
                    }).catch((pluginErr: Error) => {
                        this._logger.warn(`Fail to read plugin data in "${folder}". Plugin will be ignored. Error: ${pluginErr.message}`);
                        toBeRemoved.push(plugin);
                        return Promise.resolve();
                    });
                })).catch((readErr: Error) => {
                    this._logger.warn(`Error during reading plugins: ${readErr.message}`);
                }).finally(() => {
                    ServiceElectronService.logStateToRender(`Removing invalid plugins...`);
                    if (ServiceEnv.get().CHIPMUNK_PLUGINS_NO_REMOVE_NOTVALID) {
                        if (toBeRemoved.length > 0) {
                            this._logger.debug(`Found ${toBeRemoved.length} not valid plugins to be removed. But because CHIPMUNK_PLUGINS_NO_REMOVE_NOTVALID=true, plugins will not be removed. Not valid plugins:\n${toBeRemoved.map((plugin: InstalledPlugin) => {
                                return `\t - ${plugin.getPath()}`;
                            }).join('\n')}`);
                        }
                        return resolve();
                    } else {
                        Promise.all(toBeRemoved.map((plugin: InstalledPlugin) => {
                            return plugin.remove().then(() => {
                                ServiceElectronService.logStateToRender(`Plugin "${plugin.getPath()}" has been removed.`);
                                this._logger.debug(`Plugin "${plugin.getPath()}" is removed.`);
                            }).catch((removeErr: Error) => {
                                this._logger.warn(`Fail remove plugin "${plugin.getPath()}" due error: ${removeErr.message}`);
                                return Promise.resolve();
                            });
                        })).catch((removeErr: Error) => {
                            this._logger.warn(`Error during removing plugins: ${removeErr.message}`);
                        }).finally(() => {
                            resolve();
                        });
                    }
                });
            }).catch((error: Error) => {
                this._logger.error(`Fail to read plugins folder (${pluginStorageFolder}) due error: ${error.message}.`);
                resolve();
            });
        });
    }

    public destroy(): Promise<void> {
        return new Promise((resolve) => {
            Promise.all(Array.from(this._plugins.values()).map((plugin: InstalledPlugin) => {
                return plugin.destroy();
            })).catch((error: Error) => {
                this._logger.warn(`Error on destroy of plugin's storage: ${error.message}`);
            }).finally(() => {
                this._plugins.clear();
                resolve();
            });
        });
    }

    public shutdown(): Promise<void> {
        return new Promise((resolve) => {
            Promise.all(Array.from(this._plugins.values()).map((plugin: InstalledPlugin) => {
                return plugin.shutdown();
            })).catch((error: Error) => {
                this._logger.warn(`Error on shutdown of plugin's storage: ${error.message}`);
            }).finally(() => {
                resolve();
            });
        });
    }

    public add(name: string): Promise<void> {
        return new Promise((resolve, reject) => {
            this._store.delivery(name).then(() => {
                this._installing.set(name, name);
                resolve();
            }).catch((deliveryErr: Error) => {
                reject(deliveryErr);
            });
        });
    }

    public uninstall(name: string): Promise<void> {
        return new Promise((resolve, reject) => {
            if (!this._plugins.has(name)) {
                return reject(new Error(`Fail to find plugin "${name}"`));
            }
            this._uninstalling.set(name, name);
            resolve();
        });
    }

    public getPluginById(id: number): InstalledPlugin | undefined {
        return Array.from(this._plugins.values()).find((plugin: InstalledPlugin) => {
            return plugin.getId() === id;
        });
    }

    public getPluginByToken(token: string): InstalledPlugin | undefined {
        return Array.from(this._plugins.values()).find((plugin: InstalledPlugin) => {
            return plugin.getToken() === token;
        });
    }

    public bindWithSession(session: string, connectionFactory: TConnectionFactory): Promise<void> {
        return new Promise((resolve, reject) => {
            return Promise.all(Array.from(this._plugins.values()).map((plugin: InstalledPlugin) => {
                return plugin.bindWithSession(session, connectionFactory).then((error: Error | undefined) => {
                    if (error instanceof Error) {
                        this._logger.debug(`Plugin "${plugin.getName()}" wouldn't be attached because: ${error.message}`);
                    }
                    return Promise.resolve();
                }).catch((bindErr: Error) => {
                    this._logger.warn(`Fail bind plugin ${plugin.getName()} with session "${session}" due error: ${bindErr.message}`);
                    this._logger.warn(`Plugin ${plugin.getName()} will be excluded.`);
                    this._exclude(plugin);
                    return Promise.resolve();
                });
            })).then(() => {
                resolve();
            }).catch((error: Error) => {
                reject(error);
            });
        });
    }

    public unbindWithSession(session: string): Promise<void> {
        return new Promise((resolve, reject) => {
            return Promise.all(Array.from(this._plugins.values()).map((plugin: InstalledPlugin) => {
                return plugin.unbindWithSession(session).catch((bindErr: Error) => {
                    this._logger.warn(`Fail unbind plugin ${plugin.getName()} with session "${session}" due error: ${bindErr.message}`);
                    return Promise.resolve();
                });
            })).then(() => {
                resolve();
            }).catch((error: Error) => {
                reject(error);
            });
        });
    }

    public getNamesOfInstalled(): string[] {
        return Array.from(this._plugins.values()).map((plugin: InstalledPlugin) => {
            return plugin.getName();
        });
    }

    public getPluginRendersInfo(): IPCMessages.IRenderMountPluginInfo[] {
        const plugins: IPCMessages.IRenderMountPluginInfo[] = [];
        this._plugins.forEach((plugin: InstalledPlugin) => {
            const controller: ControllerPluginRender | undefined = plugin.getRenderController();
            if (controller === undefined || controller.getEntrypoint() === undefined) {
                return;
            }
            plugins.push({
                name: plugin.getName(),
                location: controller.getEntrypoint() as string,
                token: plugin.getToken(),
                id: plugin.getId(),
                displayName: plugin.getDisplayName(),
            });
        });
        return plugins;
    }

    public hasToBeUpdatedOrInstalled(): boolean {
        // Step 1. Check is all defaults plugins are installed or not
        const required: IPluginReleaseInfo[] = this._getRequiredDefaults();
        if (required.length > 0) {
            return true;
        }
        // Step 2. Should some plugins be updated or not
        let update: boolean = false;
        Array.from(this._plugins.values()).forEach((plugin: InstalledPlugin) => {
            if (update) {
                return;
            }
            update = plugin.isUpdateRequired();
        });
        return update;
    }

    public predownload() {
        Array.from(this._plugins.values()).forEach((plugin: InstalledPlugin) => {
            plugin.predownload();
        });
    }

    public update(): Promise<void> {
        return new Promise((resolve, reject) => {
            if (ServiceEnv.get().CHIPMUNK_PLUGINS_NO_UPDATES) {
                this._logger.debug(`Checking of plugin's updates is skipped because envvar CHIPMUNK_PLUGINS_NO_UPDATES is true`);
                return resolve();
            }
            this._logger.debug(`Updating of installed plugins is started`);
            ServiceElectronService.logStateToRender(`Updating plugins...`);
            Promise.all(Array.from(this._plugins.values()).map((plugin: InstalledPlugin) => {
                return plugin.update().then(() => {
                    ServiceElectronService.logStateToRender(`Plugin "${plugin.getPath()}" has been updated.`);
                }).catch((updateErr: Error) => {
                    this._logger.warn(`Fail to update plugin "${plugin.getName()}" due error: ${updateErr.message}`);
                    return Promise.resolve();
                });
            })).then(() => {
                this._logger.debug(`Updating of installed plugins is finished`);
                resolve();
            }).catch((error: Error) => {
                reject(new Error(this._logger.warn(`Fail to update plugins due error: ${error.message}`)));
            });
        });
    }

    public defaults(): Promise<void> {
        return new Promise((resolve) => {
            if (ServiceEnv.get().CHIPMUNK_PLUGINS_NO_DEFAULTS) {
                this._logger.debug(`Checking defaults plugins is skipped because envvar CHIPMUNK_PLUGINS_NO_DEFAULTS is true`);
                return resolve();
            }
            const required: IPluginReleaseInfo[] = this._getRequiredDefaults();
            if (required.length === 0) {
                return resolve();
            }
            this._logger.debug(`Installing default plugins`);
            this._install(required).catch((error: Error) => {
                this._logger.warn(`Error during installation of plugins: ${error.message}`);
            }).finally(() => {
                resolve();
            });
        });
    }

    public installPending(): Promise<void> {
        return new Promise((resolve, reject) => {
            if (this._installing.size === 0) {
                resolve();
            }
            const pedning: IPluginReleaseInfo[] = [];
            this._installing.forEach((name: string) => {
                const plugin: IPluginReleaseInfo | undefined = this._store.getInfo(name);
                if (plugin === undefined) {
                    this._logger.warn(`Fail find info for plugin "${name}" in store`);
                } else {
                    pedning.push(plugin);
                }
            });
            if (pedning.length === 0) {
                return resolve();
            }
            this._logger.debug(`Installing pending plugins`);
            this._install(pedning).catch((error: Error) => {
                this._logger.warn(`Error during installation of plugins: ${error.message}`);
            }).finally(() => {
                resolve();
            });
        });
    }

    public uninstallPending(): Promise<void> {
        return new Promise((resolve, reject) => {
            if (this._uninstalling.size === 0) {
                resolve();
            }
            const pedning: InstalledPlugin[] = [];
            this._uninstalling.forEach((name: string) => {
                const plugin: InstalledPlugin | undefined = this._plugins.get(name);
                if (plugin === undefined) {
                    this._logger.warn(`Fail find plugin "${name}" in storage`);
                } else {
                    pedning.push(plugin);
                }
            });
            this._logger.debug(`Uninstalling pending plugins`);
            this._uninstall(pedning).catch((error: Error) => {
                this._logger.warn(`Error during uninstalling of plugins: ${error.message}`);
            }).finally(() => {
                resolve();
            });
        });
    }

    public runAllSingleProcess(): Promise<void> {
        return new Promise((resolve, reject) => {
            Promise.all(Array.from(this._plugins.values()).filter((plugin: InstalledPlugin) => {
                return plugin.isSingleProcess();
            }).map((plugin: InstalledPlugin) => {
                return (plugin.runAsSingle() as Promise<void>).catch((error: Error) => {
                    this._logger.warn(`Fail to run as single plugin ${plugin.getName()} due error: ${error}.`);
                    this._exclude(plugin);
                    return Promise.resolve();
                });
            })).then(() => {
                this._logger.debug(`Single process plugins running is done`);
                resolve();
            }).catch((error: Error) => {
                reject(error);
            });
        });
    }

    public getInstalled(): CommonInterfaces.Plugins.IPlugin[] {
        return Array.from(this._plugins.values()).map((plugin: InstalledPlugin) => {
            const info: IInstalledPluginInfo | undefined = plugin.getInfo();
            if (info === undefined) {
                return null;
            }
            return {
                name: info.name,
                url: info.url,
                file: info.file,
                version: info.version,
                display_name: info.display_name,
                description: info.description,
                readme: info.readme,
                icon: info.icon,
                default: this._store.isDefault(info.name),
                dependencies: info.dependencies,
            };
        }).filter((data: CommonInterfaces.Plugins.IPlugin | null) => {
            return data !== null;
        }) as CommonInterfaces.Plugins.IPlugin[];
    }

    public logState() {
        this._logger.debug(`Next plugins are available:\n${Array.from(this._plugins.values()).map((plugin: InstalledPlugin) => {
            return `\t - ${plugin.getName()}`;
        }).join('\n')}`);
    }

    private _install(plugins: IPluginReleaseInfo[]): Promise<void> {
        return new Promise((resolve, reject) => {
            Promise.all(plugins.map((info: IPluginReleaseInfo) => {
                const plugin: InstalledPlugin = new InstalledPlugin(info.name, path.resolve(ServicePaths.getPlugins(), info.name), this._store);
                return plugin.install().then(() => {
                    this._plugins.set(info.name, plugin);
                }).catch((installErr: Error) => {
                    this._logger.warn(`Fail to isntall plugin "${info.name}" due error: ${installErr.message}`);
                    return Promise.resolve();
                });
            })).catch((error: Error) => {
                reject(error);
            }).finally(() => {
                resolve();
            });
        });
    }

    private _uninstall(plugins: InstalledPlugin[]): Promise<void> {
        return new Promise((resolve, reject) => {
            Promise.all(plugins.map((plugin: InstalledPlugin) => {
                return plugin.remove();
            })).then(() => {
                resolve();
            }).catch((error: Error) => {
                reject(error);
            });
        });
    }
    private _exclude(plugin: InstalledPlugin) {
        plugin.destroy().then(() => {
            this._logger.debug(`Plugin "${plugin.getName()}" was excluded.`);
        }).catch((error: Error) => {
            this._logger.debug(`Fail to correctly exclude plugin "${plugin.getName()}" due error: ${error.message}`);
        });
        this._plugins.delete(plugin.getName());
    }

    private _getRequiredDefaults(): IPluginReleaseInfo[] {
        const installed: string[] = Array.from(this._plugins.values()).map((plugin: InstalledPlugin) => {
            return plugin.getName();
        }).filter((name: string | undefined) => {
            return name !== undefined;
        });
        return this._store.getDefaults(installed);
    }

}
