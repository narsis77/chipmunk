// tslint:disable: max-classes-per-file

import * as path from 'path';
import * as FS from '../../tools/fs';
import * as tar from 'tar';
import * as semver from 'semver';
import * as Tools from '../../tools/index';

import Logger from '../../tools/env.logger';
import ControllerPluginPackage, { IPackageJson } from './plugin.package';
import ControllerPluginStore, { IPluginReleaseInfo } from './plugins.store';
import ControllerPluginRender from './plugin.controller.render';
import ControllerPluginProcess, { TConnectionFactory } from './plugin.controller.process';
import ControllerIPCPlugin from './plugin.process.ipc';
import ServicePaths from '../../services/service.paths';
import ServiceElectronService from '../../services/service.electron.state';
import ServicePackage from '../../services/service.package';

import { CommonInterfaces } from '../../interfaces/interface.common';

export { IPackageJson, TConnectionFactory };

export type TPluginName = string;

export interface IInstalledPluginInfo {
    name: string;                                               // Included into "info.json"
    url: string;                                                // Included into "info.json"
    file: string;                                               // Included into "info.json"
    version: string;                                            // Included into "info.json"
    hash: string;                                               // Included into "info.json"
    phash: string;                                              // Included into "info.json"
    dependencies: CommonInterfaces.Versions.IDependencies;      // Included into "info.json"
    display_name: string;                                       // Included into "info.json"
    description: string;                                        // Included into "info.json"
    readme: string;                                             // Included into "info.json"
    icon: string;                                               // Included into "info.json"
    package: {
        render: ControllerPluginPackage | undefined;
        process: ControllerPluginPackage | undefined;
    };
    controller: {
        render: ControllerPluginRender | undefined;
        process: ControllerPluginProcess | undefined;
    };
}

const CPluginInfoFile: string = 'info.json';

const CPluginsFolders = {
    process: 'process',
    render: 'render',
};

export default class ControllerPluginInstalled {

    private _logger: Logger;
    private _path: string;
    private _name: string;
    private _info: IInstalledPluginInfo | undefined;
    private _store: ControllerPluginStore;
    private _token: string = Tools.guid();
    private _id: number = Tools.sequence();

    constructor(_name: string, _path: string, store: ControllerPluginStore) {
        this._name = _name;
        this._path = _path;
        this._store = store;
        this._logger = new Logger(`ControllerPluginInstalled (${this._path})`);
    }

    public destroy(): Promise<void> {
        return new Promise((resolve) => {
            if (this._info === undefined || this._info.controller === undefined || this._info.controller.process === undefined) {
                return resolve();
            }
            return this._info.controller.process.destroy().then(() => {
                if (this._info === undefined || this._info.controller === undefined || this._info.controller.process === undefined) {
                    return;
                }
                this._info.controller.process = undefined;
            }).catch((error: Error) => {
                this._logger.warn(`Error during destroy plugin's process: ${error.message}`);
            }).finally(() => {
                resolve();
            });
        });
    }

    public shutdown(): Promise<void> {
        return new Promise((resolve) => {
            if (this._info === undefined || this._info.controller === undefined || this._info.controller.process === undefined) {
                return resolve();
            }
            return this._info.controller.process.destroy().then(() => {
                if (this._info === undefined || this._info.controller === undefined || this._info.controller.process === undefined) {
                    return;
                }
                this._info.controller.process = undefined;
            }).catch((error: Error) => {
                this._logger.warn(`Error during shutdown plugin's process: ${error.message}`);
            }).finally(() => {
                resolve();
            });
        });
    }

    public read(): Promise<void> {
        return new Promise((resolve, reject) => {
            const filename: string = path.resolve(this._path, CPluginInfoFile);
            ServiceElectronService.logStateToRender(`Reading plugin data "${path.basename(this._path)}"`);
            FS.exist(filename).then((exist: boolean) => {
                if (!exist) {
                    return reject(new Error(this._logger.warn(`Not valid plugin. Info-file "${filename}" doesn't exist`)));
                }
                FS.readTextFile(filename).then((content: string) => {
                    try {
                        this._info = JSON.parse(content);
                    } catch (e) {
                        this._info = undefined;
                        this._logger.warn(`Fail parse info-file due error: ${e.message}`);
                        return reject(e);
                    }
                    if (this._info?.phash !== ServicePackage.getHash(this._info?.dependencies)) {
                        this._logger.warn(`Plugin could not be used, because hash dismatch.\n\t- plugin hash: ${this._info?.hash}\n\t- chipmunk hash: ${ServicePackage.getHash()}`);
                        return reject(new Error(`Version-hash dismatch`));
                    }
                    ServiceElectronService.logStateToRender(`Reading plugin package "${path.basename(this._path)}"`);
                    this._readPackages().then(() => {
                        if (this._info?.package.render === undefined && this._info?.package.process === undefined) {
                            this._info = undefined;
                            return reject(new Error(this._logger.warn(`Plugin doesn't have valid [render] and [process]. Plugin will not be used.`)));
                        }
                        ServiceElectronService.logStateToRender(`Creating controllers for "${path.basename(this._path)}"`);
                        this._addControllers().then(() => {
                            this._logPluginState();
                            resolve();
                        }).catch((controllersErr: Error) => {
                            reject(controllersErr);
                        });
                    }).catch((packageJsonErr: Error) => {
                        this._info = undefined;
                        reject(new Error(this._logger.warn(`Error during reading package.json of plugin: ${packageJsonErr.message}. Plugin will not be used.`)));
                    });
                }).catch((readingErr: Error) => {
                    this._logger.warn(`Fail read info-file due error: ${readingErr.message}`);
                    reject(readingErr);
                });
            }).catch((err: Error) => {
                this._logger.warn(`Fail check info-file due error: ${err.message}`);
                reject(err);
            });
        });
    }

    public isValid(): boolean {
        return this._info !== undefined;
    }

    public getInfo(): IInstalledPluginInfo | undefined {
        return this._info;
    }

    public getName(): string {
        if (this._info === undefined) {
            this._logger.error(`Attempt to get name of plugin, which isn't read or not valid.`);
        }
        return this._info?.name as string;
    }

    public getDisplayName(): string {
        if (this._info === undefined) {
            this._logger.error(`Attempt to get display_name of plugin, which isn't read or not valid.`);
        }
        return this._info?.display_name as string;
    }

    public getPath(): string {
        return this._path;
    }

    public getId(): number {
        return this._id;
    }

    public getToken(): string {
        return this._token;
    }

    public getSessionIPC(session: string): ControllerIPCPlugin | undefined {
        if (this._info === undefined || this._info.controller === undefined || this._info.controller.process === undefined) {
            return undefined;
        }
        return this._info.controller.process.getSessionIPC(session);
    }

    public getRenderController(): ControllerPluginRender | undefined {
        if (this._info === undefined || this._info.controller === undefined || this._info.controller.render === undefined) {
            return undefined;
        }
        return this._info.controller.render;
    }

    public remove(): Promise<void> {
        return new Promise((resolve, reject) => {
            ServiceElectronService.logStateToRender(`Removing plugin "${path.basename(this._path)}"`);
            FS.rmdir(this._path).then(() => {
                ServiceElectronService.logStateToRender(`Plugin "${path.basename(this._path)}" has been removed`);
                resolve();
            }).catch((error: Error) => {
                this._logger.warn(`Fail to remove file due error: ${error.message}`);
                reject(error);
            });
        });
    }

    public isUpdateRequired(): boolean {
        if (this._info === undefined) {
            return false;
        }
        const available: IPluginReleaseInfo | undefined = this._store.getInfo(this._name);
        if (available === undefined) {
            return false;
        }
        if (ServicePackage.getHash(available.dependencies) !== available.phash) {
            return false;
        }
        if (this._info.phash === available.phash && this._info.version === available.version) {
            return false;
        }
        return true;
    }

    public predownload() {
        if (!this.isUpdateRequired()) {
            return;
        }
        this._store.delivery(this._name).then((filename: string) => {
            this._logger.env(`Updated version of plugins is deliveried to "${filename}"`);
        }).catch((error: Error) => {
            this._logger.warn(`Fail to delivery plugin due error: ${error.message}`);
        });
    }

    public update(): Promise<void> {
        return new Promise((resolve, reject) => {
            if (this._info === undefined) {
                return reject(new Error(this._logger.warn(`Cannot update plugin, because it isn't initialized.`)));
            }
            const available: IPluginReleaseInfo | undefined = this._store.getInfo(this._name);
            if (available === undefined) {
                this._logger.warn(`Plugin will not be update, because there are no such plugin in store`);
                return resolve();
            }
            if (ServicePackage.getHash(available.dependencies) !== available.phash) {
                this._logger.warn(`Plugin will not be updated, because hash dismatch.\n\t- plugin hash: ${available.phash}\n\t- chipmunk hash: ${ServicePackage.getHash(available.dependencies)}`);
                return resolve();
            }
            if (this._info.phash === available.phash && this._info.version === available.version) {
                this._logger.debug(`Update of plugin isn't required.`);
                return resolve();
            }
            // Remove current version of plugin
            this.remove().then(() => {
                this._logger.debug(`Plugin is removed. New version will be downloaded`);
                ServiceElectronService.logStateToRender(`Updating plugin "${path.basename(this._path)}"...`);
                // Download updated version of plugin
                this._store.download(this._name).then((filename: string) => {
                    this._logger.debug(`New version of plugin is downloaded: ${filename}`);
                    ServiceElectronService.logStateToRender(`Unpacking package of plugin "${path.basename(this._path)}"`);
                    // Unpack plugin
                    this._unpack(filename).then(() => {
                        this._logger.debug(`Plugin is unpacked`);
                        ServiceElectronService.logStateToRender(`Plugin "${path.basename(this._path)}" has been unpacked`);
                        // Read plugin info once again
                        this.read().then(() => {
                            this._logger.debug(`Plugin is successfully updated`);
                            resolve();
                        }).catch((readErr: Error) => {
                            reject(new Error(this._logger.warn(`Fail to updated plugin due error: ${readErr.message}`)));
                        });
                    }).catch((unpackErr: Error) => {
                        reject(new Error(this._logger.warn(`Fail to unpack plugin due error: ${unpackErr.message}`)));
                    });
                }).catch((downloadErr: Error) => {
                    reject(new Error(`Fail to download new version of plugin due error: ${downloadErr.message}`));
                });
            }).catch((removeErr: Error) => {
                reject(new Error(this._logger.warn(`Fail to remove plugin due error: ${removeErr.message}`)));
            });
        });
    }

    public install(): Promise<void> {
        return new Promise((resolve, reject) => {
            const available: IPluginReleaseInfo | undefined = this._store.getInfo(this._name);
            if (available === undefined) {
                return reject(new Error(this._logger.warn(`Plugin will not be installed, because there are no such plugin in store`)));
            }
            // Download plugin
            this._store.download(this._name).then((filename: string) => {
                this._logger.debug(`Plugin is downloaded: ${filename}`);
                ServiceElectronService.logStateToRender(`Unpacking package of plugin "${path.basename(this._path)}"`);
                // Unpack plugin
                this._unpack(filename).then(() => {
                    ServiceElectronService.logStateToRender(`Plugin "${path.basename(this._path)}" has been unpacked`);
                    this._logger.debug(`Plugin is unpacked`);
                    // Read plugin info once again
                    this.read().then(() => {
                        this._logger.debug(`Plugin is successfully installed`);
                        resolve();
                    }).catch((readErr: Error) => {
                        reject(new Error(this._logger.warn(`Fail to install plugin due error: ${readErr.message}`)));
                    });
                }).catch((unpackErr: Error) => {
                    reject(new Error(this._logger.warn(`Fail to unpack plugin due error: ${unpackErr.message}`)));
                });
            }).catch((downloadErr: Error) => {
                reject(new Error(`Fail to download plugin due error: ${downloadErr.message}`));
            });
        });
    }

    public isSingleProcess(): boolean {
        if (this._info === undefined) {
            return false;
        }
        if (this._info.controller === undefined) {
            return false;
        }
        if (this._info.controller.process === undefined) {
            return false;
        }
        return this._info.controller.process.isSingleProcess();
    }

    public runAsSingle(): Promise<void> | Error {
        if (this._info === undefined) {
            return new Error(`Plugin isn't inited`);
        }
        if (this._info.controller === undefined) {
            return new Error(`Plugin's controllers arn't inited`);
        }
        if (this._info.controller.process === undefined) {
            return new Error(`Plugin doesn't have process part`);
        }
        if (!this._info.controller.process.isSingleProcess()) {
            return new Error(`Plugin isn't single process`);
        }
        return this._info.controller.process.runAsSingle();
    }

    public bindWithSession(session: string, connectionFactory: TConnectionFactory): Promise<Error | undefined> {
        return new Promise((resolve, reject) => {
            if (this._info === undefined) {
                return resolve(new Error(`Plugin isn't inited`));
            }
            if (this._info.controller === undefined) {
                return resolve(new Error(`Plugin's controllers arn't inited`));
            }
            if (this._info.controller.process === undefined) {
                return resolve(new Error(`Plugin doesn't have process part`));
            }
            if (this._info.controller.process.isSingleProcess()) {
                this._info.controller.process.bindSinglePlugin(session, connectionFactory).then(() => {
                    resolve(undefined);
                }).catch(reject);
            } else if (this._info.controller.process.isMultipleProcess()) {
                this._info.controller.process.bindMultiplePlugin(session, connectionFactory).then(() => {
                    resolve(undefined);
                }).catch(reject);
            }
        });
    }

    public unbindWithSession(session: string): Promise<void> {
        return new Promise((resolve, reject) => {
            if (this._info === undefined) {
                return reject(new Error(`Plugin isn't inited`));
            }
            if (this._info.controller === undefined) {
                return reject(new Error(`Plugin's controllers arn't inited`));
            }
            if (this._info.controller.process === undefined) {
                return reject(new Error(`Plugin doesn't have process part`));
            }
            if (this._info.controller.process.isSingleProcess()) {
                this._info.controller.process.unbindSingle(session).then(resolve).catch(reject);
            } else if (this._info.controller.process.isMultipleProcess()) {
                this._info.controller.process.unbindMuliple(session).then(resolve).catch(reject);
            }
        });
    }
    private _logPluginState() {
        let msg = `Plugin state:\n`;
        if (this._info === undefined) {
            msg += `\tNOT READY`;
        } else {
            if (this._info.package === undefined) {
                msg += `\tpackages: NOT READ\n`;
            } else {
                msg += `\tpackage render:\t\t${this._info.package.render !== undefined ? 'OK' : '-'}\n\tpackage process:\t${this._info.package.process !== undefined ? 'OK' : '-'}\n`;
            }
            if (this._info.controller === undefined) {
                msg += `\tcontrollers:\tNOT INIT`;
            } else {
                msg += `\tcontroller render:\t${this._info.controller.render !== undefined ? 'OK' : '-'}\n\tcontroller process:\t${this._info.controller.process !== undefined ? 'OK' : '-'}`;
            }
        }
        this._logger.env(msg);
    }

    private _unpack(tgzfile: string, removetgz: boolean = true): Promise<string> {
        return new Promise((resolve, reject) => {
            tar.x({
                file: tgzfile,
                cwd: ServicePaths.getPlugins(),
            }).then(() => {
                if (!removetgz) {
                    return resolve(ServicePaths.getPlugins());
                }
                FS.unlink(tgzfile).catch((removeErr: Error) => {
                    this._logger.warn(`Fail to remove ${tgzfile} due error: ${removeErr.message}`);
                }).finally(() => {
                    resolve(ServicePaths.getPlugins());
                });
            }).catch(reject);
        });
    }

    private _readPackages(): Promise<void> {
        return new Promise((resolve, reject) => {
            if (this._info === undefined) {
                return reject(`Basic info hadn't been read`);
            }
            this._info.package = {
                render: undefined,
                process: undefined,
            };
            const render = new ControllerPluginPackage(path.resolve(this._path, CPluginsFolders.render), this._name);
            const process = new ControllerPluginPackage(path.resolve(this._path, CPluginsFolders.process), this._name);
            Promise.all([
                render.read().then(() => {
                    (this._info as IInstalledPluginInfo).package.render = render;
                }).catch(() => {
                    return Promise.resolve();
                }),
                process.read().then(() => {
                    (this._info as IInstalledPluginInfo).package.process = process;
                }).catch(() => {
                    return Promise.resolve();
                }),
            ]).catch((error: Error) => {
                this._logger.debug(`Error reading package.json: ${error.message}`);
            }).finally(() => {
                resolve();
            });
        });
    }

    private _addControllers(): Promise<void> {
        return new Promise((resolve, reject) => {
            if (this._info === undefined) {
                return reject(`Basic info hadn't been read`);
            }
            if (this._info.package === undefined) {
                return reject(`Packages hadn't been read`);
            }
            const tasks = [];
            this._info.controller = {
                render: undefined,
                process: undefined,
            };
            if (this._info.package.render !== undefined) {
                const render = new ControllerPluginRender(this._name, this._info.package.render);
                tasks.push(render.init().then(() => {
                    (this._info as IInstalledPluginInfo).controller.render = render;
                }).catch((error: Error) => {
                    this._logger.warn(`Fail to init render controller due error: ${error.message}`);
                    if (this._info === undefined) {
                        return;
                    }
                    this._info.controller.render = undefined;
                }));
            }
            if (this._info.package.process !== undefined) {
                const process = new ControllerPluginProcess(this._name, this._token, this._id, this._info.package.process);
                tasks.push(process.init().then(() => {
                    (this._info as IInstalledPluginInfo).controller.process = process;
                }).catch((error: Error) => {
                    this._logger.warn(`Fail to init process controller due error: ${error.message}`);
                    if (this._info === undefined) {
                        return;
                    }
                    this._info.controller.process = undefined;
                }));
            }
            Promise.all(tasks).then(() => {
                resolve();
            }).catch((initErr: Error) => {
                reject(initErr);
            });
        });
    }

}
