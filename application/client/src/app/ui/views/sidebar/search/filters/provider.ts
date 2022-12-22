import { Entity } from '../providers/definitions/entity';
import { Provider } from '../providers/definitions/provider';
import { FilterRequest } from '@service/session/dependencies/search/filters/store';
import { DisabledRequest } from '@service/session/dependencies/search/disabled/request';
import { IComponentDesc } from '@elements/containers/dynamic/component';
import { FiltersList } from './list/component';
import { FiltersPlaceholder } from './placeholder/component';
import { FilterDetails } from './details/component';
import { IMenuItem } from '@ui/service/contextmenu';
import { DragableRequest, ListContent } from '../draganddrop/service';
import { CdkDragDrop } from '@angular/cdk/drag-drop';
import { EntityData } from '../providers/definitions/entity.data';

export class ProviderFilters extends Provider<FilterRequest> {
    private _entities: Map<string, Entity<FilterRequest>> = new Map();
    private _listID: ListContent = ListContent.filtersList;

    public init(): void {
        this.updatePanels();
        this.subscriber.register(
            this.session.search
                .store()
                .filters()
                .subjects.get()
                .value.subscribe(() => {
                    super.change();
                    this.session.search
                        .state()
                        .filters()
                        .catch((err: Error) => {
                            console.log(err);
                        });
                }),
        );
        this.subscriber.register(
            this.session.search.subjects.get().updated.subscribe((event) => {
                this._entities.forEach((entity) => {
                    const alias = entity.extract().alias();
                    entity
                        .extract()
                        .set()
                        .found(event.stat[alias] === undefined ? 0 : event.stat[alias]);
                });
            }),
        );
    }

    public entities(): Array<Entity<FilterRequest>> {
        const guids: string[] = [];
        const entities = this.session.search
            .store()
            .filters()
            .get()
            .map((filter: FilterRequest) => {
                let entity = this._entities.get(filter.definition.uuid);
                if (entity === undefined) {
                    entity = new Entity<FilterRequest>(filter);
                } else {
                    entity.set(filter);
                }
                this._entities.set(filter.definition.uuid, entity);
                guids.push(filter.definition.uuid);
                return entity;
            });
        this._entities.forEach((_, guid: string) => {
            if (guids.indexOf(guid) === -1) {
                this._entities.delete(guid);
            }
        });
        return entities;
    }

    public reorder(params: { prev: number; curt: number }) {
        this.session.search.store().filters().reorder(params);
        super.change();
    }

    public getPanels(): {
        list(): {
            name(): string;
            desc(): string;
            comp(): IComponentDesc;
        };
        details(): {
            name(): string | undefined;
            desc(): string | undefined;
            comp(): IComponentDesc | undefined;
        };
        nocontent(): {
            name(): string | undefined;
            desc(): string | undefined;
            comp(): IComponentDesc | undefined;
        };
    } {
        return {
            list: (): {
                name(): string;
                desc(): string;
                comp(): IComponentDesc;
            } => {
                return {
                    name: (): string => {
                        return `Filters`;
                    },
                    desc: (): string => {
                        const count = this.entities().length;
                        return `${count} filter${count > 1 ? 's' : ''}`;
                    },
                    comp: (): IComponentDesc => {
                        return {
                            factory: FiltersList,
                            inputs: {
                                provider: this,
                                draganddrop: this.draganddrop,
                                session: this.session,
                            },
                        };
                    },
                };
            },
            details: (): {
                name(): string | undefined;
                desc(): string | undefined;
                comp(): IComponentDesc | undefined;
            } => {
                return {
                    name: (): string | undefined => {
                        return `Filter Details`;
                    },
                    desc: (): string | undefined => {
                        if (this.select().get().length !== 1) {
                            return '';
                        }
                        const selection = this._entities.get(this.select().get()[0]);
                        if (selection === undefined) {
                            return '';
                        }
                        return selection.extract().definition.filter.filter;
                    },
                    comp: (): IComponentDesc | undefined => {
                        return {
                            factory: FilterDetails,
                            inputs: {
                                provider: this,
                                draganddrop: this.draganddrop,
                            },
                        };
                    },
                };
            },
            nocontent: (): {
                name(): string | undefined;
                desc(): string | undefined;
                comp(): IComponentDesc | undefined;
            } => {
                return {
                    name: (): string | undefined => {
                        return `Filters`;
                    },
                    desc: (): string | undefined => {
                        return undefined;
                    },
                    comp: (): IComponentDesc | undefined => {
                        return {
                            factory: FiltersPlaceholder,
                            inputs: {
                                provider: this,
                                draganddrop: this.draganddrop,
                            },
                        };
                    },
                };
            },
        };
    }

    public getContextMenuItems(target: Entity<any>, selected: Array<Entity<any>>): IMenuItem[] {
        if (selected.length !== 1) {
            return [];
        }
        const entity = selected[0].extract();
        const items: IMenuItem[] = [];
        if (entity instanceof FilterRequest) {
            items.push({
                caption: `Show Matches`,
                handler: () => {
                    this.search(selected[0]);
                },
            });
        }
        return items;
    }

    public search(entity: Entity<FilterRequest>) {
        this.session.search
            .state()
            .setActive(entity.extract().definition.filter)
            .catch((error: Error) => {
                this.logger.error(`Fail to make search: ${error.message}`);
            });
    }

    public actions(
        target: Entity<any>,
        selected: Array<Entity<any>>,
    ): {
        activate?: () => void;
        deactivate?: () => void;
        remove?: () => void;
        edit?: () => void;
    } {
        const actions: {
            activate?: () => void;
            deactivate?: () => void;
            remove?: () => void;
            edit?: () => void;
        } = {};
        const self = this;
        const entities = selected.filter((entity: Entity<any>) => {
            return entity.extract() instanceof FilterRequest;
        });
        actions.activate =
            entities.filter((entity: Entity<FilterRequest>) => {
                return entity.extract().definition.active === false;
            }).length !== 0
                ? () => {
                      entities.forEach((entity: Entity<FilterRequest>) => {
                          entity.extract().set().state(true);
                      });
                  }
                : undefined;
        actions.deactivate =
            entities.filter((entity: Entity<FilterRequest>) => {
                return entity.extract().definition.active === true;
            }).length !== 0
                ? () => {
                      entities.forEach((entity: Entity<FilterRequest>) => {
                          entity.extract().set().state(false);
                      });
                  }
                : undefined;
        actions.edit =
            selected.length === 1 && entities.length === 1
                ? () => {
                      // View should be focused to switch to edit-mode, but while context
                      // menu is open, there are no focus. Well, that's why settimer here.
                      setTimeout(() => {
                          self.edit().in();
                      });
                  }
                : undefined;
        actions.remove =
            entities.length !== 0
                ? () => {
                      if (entities.length === self.entities().length) {
                          this.session.search
                              .store()
                              .filters()
                              .clear()
                              .catch((error: Error) => {
                                  this.logger.error(`Fail to clear store: ${error.message}`);
                              });
                          self.change();
                      } else {
                          entities.forEach((entity: Entity<FilterRequest>) => {
                              this.session.search.store().filters().delete([entity.uuid()]);
                          });
                      }
                  }
                : undefined;
        return actions;
    }

    public isVisable(): boolean {
        const dragging: Entity<DragableRequest> = this.draganddrop.dragging;
        if (dragging) {
            const request: DragableRequest = dragging.extract();
            if (request instanceof DisabledRequest) {
                if ((request as DisabledRequest).entity() instanceof FilterRequest) {
                    return true;
                }
                return false;
                // } else if (request instanceof ChartRequest || request instanceof FilterRequest) {
                //     return true;
            }
        }
        return false;
    }

    public dropped(event: CdkDragDrop<EntityData<DragableRequest>>) {
        if (event.previousContainer === event.container) {
            this.reorder({ prev: event.previousIndex, curt: event.currentIndex });
        } else {
            const index: number = event.previousIndex;
            const data: EntityData<DragableRequest> = event.previousContainer.data;
            if (data.disabled !== undefined) {
                const outside: Entity<DisabledRequest> | undefined =
                    data.disabled[event.previousIndex] !== undefined
                        ? data.disabled[index]
                        : undefined;
                if (outside === undefined) {
                    return;
                }
                const disabled: DisabledRequest = outside.extract();
                this.session.search.store().disabled().delete([disabled.uuid()]);
                this.session.search.store().filters().tryRestore(disabled.entity());
                // if (data.entries !== undefined) {
                //     const outside: Entity<ChartRequest> | undefined =
                //         data.entries[event.previousIndex] !== undefined
                //             ? (data.entries[index] as Entity<ChartRequest>)
                //             : undefined;
                //     if (
                //         outside !== undefined &&
                //         typeof outside.getEntity === 'function' &&
                //         outside.extract() instanceof ChartRequest
                //     ) {
                //         session
                //             .getSessionSearch()
                //             .getChartsAPI()
                //             .getStorage()
                //             .remove(outside.extract());
                //         session
                //             .getSessionSearch()
                //             .getFiltersAPI()
                //             .getStorage()
                //             .add(
                //                 {
                //                     request: outside.extract().asDesc().request,
                //                     flags: {
                //                         casesensitive: true,
                //                         wholeword: true,
                //                         regexp: true,
                //                     },
                //                 },
                //                 event.currentIndex,
                //             );
                //     }
            }
        }
    }

    public get listID(): ListContent {
        return this._listID;
    }
}
